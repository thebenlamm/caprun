# DESIGN-confirm-binding.md — caprun Confirm-Binding to Resolved Literals (v1.3)

**Requirement:** DESIGN-01 (forward-references CONFIRM-03, CONFIRM-04)
**Status:** Draft — pending `DESIGN-GATE-RECORD-v1.3.md` approval
**Canonical source:** `planning-docs/PLAN.md` (wins on any conflict)
**Gate:** `crates/executor` MUST NOT gain CONFIRM-03 binding code, and `crates/brokerd` MUST NOT gain
the combined-digest confirm/deny code, until this document AND its companion
`planning-docs/DESIGN-content-adapter-mediation.md` are both reviewed and
`planning-docs/DESIGN-GATE-RECORD-v1.3.md` records decision = APPROVED.

**Prior art / relationship to the approved v1.2 doc:** This document extends
`planning-docs/DESIGN-confirmation-release.md` (the `PendingConfirmation`/`ResolvedArg` durable
checkpoint mechanism, APPROVED per `planning-docs/DESIGN-GATE-RECORD-v1.2.md`) and its backing
implementation, `crates/brokerd/src/confirmation.rs`. It does **not** replace either — it specifies
how `caprun confirm`/`caprun deny` bind their decision to a single combined digest over the FULL SET
of blocked args' resolved literals, a binding shape that v1.2's single-arg-block world did not need
because v1.2 never blocked more than one arg per plan node.

---

## The Problem Being Solved

v1.2's `PendingConfirmation` (`crates/brokerd/src/confirmation.rs`) persists a resolved snapshot —
`resolved_args: Vec<ResolvedArg>` — of every arg on a blocked plan node, frozen at Block time,
precisely because the confirm/deny process is always a SECOND, LATER OS process and the in-memory
`ValueStore` that resolved the original `ValueId` handles is gone by the time it runs
(`DESIGN-confirmation-release.md` "The Problem Being Solved"). That mechanism is not being replaced
here.

What v1.2 did not need — because its executor Blocked on exactly one arg per decision
(`SinkBlockedAnchor.arg: String`, singular) — is a way to bind the human's confirm/deny decision to
MORE THAN ONE literal at once. `DESIGN-content-adapter-mediation.md`'s collect-then-Block change
(D-14) makes `ExecutorDecision::BlockedPendingConfirmation`/`SinkBlockedAnchor` PLURAL: a plan node
carrying both a tainted `to` and a tainted `body` now surfaces BOTH as one combined Block. Without a
binding rule, "confirm this block" is ambiguous — confirm WHICH literal, or all of them? — and the
exact B1-reincarnation risk `DESIGN-content-adapter-mediation.md` names (a human confirms the shown
recipient, and `confirmation.rs::confirm()`'s full-`resolved_args`-snapshot re-invocation ships the
body too, unconfirmed) reappears at the binding layer if the digest does not cover the whole set.

This document specifies the fix: `caprun confirm` binds to ONE combined SHA-256 digest over the FULL
SET of blocked args' resolved literals — never a per-arg digest, never a digest over a subset — so
that "confirm" and "the set the human saw" and "the set the digest attests to" and "the set the
adapter sends" are, by construction, the same set.

---

## Combined-Digest Binding (CONFIRM-03, D-08/D-19)

**MUST:** `caprun confirm` binds to ONE combined SHA-256 digest covering the FULL SET of blocked
args' exact resolved literals — recipient AND body TOGETHER as one set, never as separate per-arg
digests — matching the collect-then-Block blocked-arg set defined in
`DESIGN-content-adapter-mediation.md`'s "Collect-then-Block (D-14)" section. Whatever set that
document's per-arg loop collects into one `BlockedPendingConfirmation` IS the set this digest covers
— the two documents MUST agree on the set shape; this document does not define its own, separate
notion of "the blocked args."

**MUST (why this shape, not two digests or a second round-trip):** ONE combined digest over the
whole set is the only shape that satisfies `DESIGN-content-adapter-mediation.md`'s D-02 (both the
routing-sensitive arg and the content-sensitive arg surface as Blocked in the same decision) and D-08
(the digest binds the resolved recipient+body literals) simultaneously, without requiring a second
confirm round-trip per arg. A per-arg digest scheme would require the human to confirm N separate
digests for an N-arg block, reopening exactly the "one confirm silently releases an unconfirmed
sibling arg" risk this design gate exists to close — a partial confirm (digest 1 confirmed, digest 2
still pending) would leave `confirmation.rs::confirm()`'s full-snapshot re-invocation with no rule for
what to do with the still-pending arg. A single combined digest has no such partial state: the set is
confirmed or it is not.

**MUST (hash primitive reuse):** The combined digest MUST reuse SHA-256, matching the existing
`literal_sha256` pattern already used for the single-arg case (`crates/executor/src/lib.rs:112-116`):

```rust
// Illustrative shape, not literal code to paste — the existing single-arg pattern this section reuses.
// Source: crates/executor/src/lib.rs:112-116 (existing code, read directly).
let literal_sha256 = {
    let mut hasher = Sha256::new();
    hasher.update(record.literal.as_bytes());
    hex::encode(hasher.finalize())
};
```

The combined-set variant of this same pattern MUST NOT introduce a new hash primitive — no HMAC, no
keyed hash, no non-SHA-256 digest. The only change from the existing single-arg pattern is the input:
instead of hashing one `record.literal`, the combined digest hashes a deterministic concatenation of
every blocked arg's resolved literal, in a MUST-fixed, canonical order (e.g., the order the collected
`Vec<BlockedArg>` was produced in by the per-arg loop, itself the stable `plan_node.args` iteration
order) — so the same blocked-arg set always produces the same digest, and the digest is exactly
reproducible by an independent verifier re-hashing `PendingConfirmation.resolved_args` in that same
order.

**MUST (schema extension — extends `PendingConfirmation`, does not replace it):** The combined digest
is a NEW field alongside the existing `resolved_args: Vec<ResolvedArg>` in `PendingConfirmation`
(`crates/brokerd/src/confirmation.rs:100-124`) — it is additive, following exactly the "frozen at
Block time, never re-derived" doc-comment discipline `ResolvedArg`'s existing fields already use
(`confirmation.rs:38-49`, e.g. `literal: String, // frozen at Block time`):

```rust
// Illustrative shape, not literal code to paste — additive to the existing struct.
struct PendingConfirmation {
    effect_id:       Uuid,
    session_id:      Uuid,
    blocked_event_id: Uuid,
    sink:            SinkId,
    resolved_args:   Vec<ResolvedArg>,   // UNCHANGED — the existing v1.2 field, now plural-populated
    combined_digest: String,             // NEW — SHA-256 over the full resolved_args set's literals,
                                          // frozen at Block time, never re-derived at confirm time
    workspace_root_path: String,
    state:           PendingConfirmationState,
}
```

`combined_digest` MUST be computed and persisted ONCE, at the same Block-time write that persists
`resolved_args` (the same atomic transaction as the `sink_blocked` Event append, per
`DESIGN-confirmation-release.md`'s existing Persistence Contract). It MUST NOT be recomputed at
confirm time from a live re-resolution of any `ValueId` — recomputing would reopen the exact
in-memory-`ValueStore`-is-gone problem `DESIGN-confirmation-release.md`'s "The Problem Being Solved"
already ruled out for `resolved_args` itself. `combined_digest` is read back, verbatim, from the
persisted row — never derived fresh by the confirm/deny process.

---

## Post-Transformation Bytes — No Drift Between Confirm and Send (CONFIRM-03, D-08, Pitfall 2)

**MUST:** The confined-worker extractor mints the `ValueRecord` (via `mint_from_read` or its
Phase-15 successor) ONLY AFTER applying any transformation to the raw read bytes — concatenation,
base64-decode, or any other EXTRACT-03 manipulation variant. Minting MUST happen post-transform, not
pre-transform.

**MUST NOT:** There MUST be NO transformation step between `ValueRecord` mint and executor Block, and
NONE between Block (frozen into `ResolvedArg.literal`) and adapter invocation. A `ValueRecord`, once
minted, is carried through Block and into `ResolvedArg.literal` byte-for-byte — no concatenation, no
decoding, no re-encoding, no normalization is ever applied to it after minting.

**Why this closes D-12(b):** Because the combined digest (above) is computed over the frozen
`ResolvedArg.literal` fields, and because those literals are guaranteed by this mint-after-transform,
no-transform-after-mint rule to already BE the exact bytes the extractor would hand to the sink, the
combined digest is guaranteed to equal the exact bytes the adapter transmits. The bytes the human
read at confirm time are provably the bytes sent — there is no window in which a transform could run
between "what the digest attests to" and "what actually goes out over SMTP." This directly answers
attack vector D-12(b) (hash over pre-transformation bytes instead of the post-transformation bytes
actually sent): the answer is not a check performed AT hash time, it is a rule that removes the
opportunity for drift to exist at all, by construction, before the hash is ever computed.

**MUST NOT (the specific anti-pattern this rules out):** A Phase-15 extractor implementation MUST NOT
resolve a `ValueId` to its literal, THEN perform a string operation (concatenation, base64-decode) on
the result, and hand that transformed string to the sink as if it were still the same `ValueRecord`.
Any transform MUST mint a FRESH `ValueRecord` for the transformed value, inheriting taint from its
inputs, BEFORE that value is ever used as a plan-node arg or reaches the executor's per-arg loop.

---

## Verbatim Display — No Truncation (CONFIRM-03, D-09)

**MUST:** The confirm/deny display shows the verbatim resolved literal for every blocked arg in the
set — no summary, no truncation of long bodies, no elision (e.g., no "...", no character-count cap,
no "show first N bytes"). The human MUST see the full bytes the combined digest attests to, for EVERY
arg in the set, not merely the first or the shortest.

**MUST NOT:** The display MUST NOT truncate a long email body to fit a terminal width, a log-line
length convention, or any other display budget. If the literal is long, the full literal is shown in
full — this is a hard MUST, not a UX nicety, because a truncated display would mean the human confirms
bytes they did not actually see, which is indistinguishable in effect from confirming the wrong value.
This mirrors `DESIGN-confirmation-release.md`'s existing "raw value, not a vague warning" discipline
(its CLI Contract section, Accepted Residual Risk 1) and extends it: what was previously true for one
literal (a filesystem path) MUST now hold independently for every literal in the combined set
(a recipient address AND a body, potentially of very different lengths).

---

## Block Narration for Every Arg (CONFIRM-04, D-20)

**MUST:** The Block moment narrates provenance for EVERY blocked arg in the set — recipient/body to
untrusted doc to these bytes to this sink arg, for each blocked arg individually. This MUST include,
per arg: the sink and arg name, the literal value verbatim, the taint labels, the source read Event id
and session id, and the `provenance_chain` summary — the same fields
`DESIGN-confirmation-release.md`'s CLI Contract section already specifies for the single-arg case,
now repeated once per arg in the set.

**MUST NOT:** The narration MUST NEVER be a bare `"Error: blocked"` with no per-arg detail, and MUST
NEVER show only the first-matched arg while silently omitting the others. This is the human-legibility
counterpart to collect-then-Block (`DESIGN-content-adapter-mediation.md`'s D-14): if the collected set
carries two tainted args (a tainted `to` and a tainted `body`), the human MUST see both literals and
both provenance chains, side by side, before deciding to confirm or deny — never a display that shows
one and asks the human to trust that the confirm also covers "whatever else was blocked."

**MUST (ordering):** Per-arg narration MUST be presented in the same canonical order the combined
digest (above) was computed over, so the displayed order and the hashed order agree — a human
manually re-deriving the digest from the display (e.g., for an independent audit) MUST be able to do
so without needing to guess an ordering convention.

---

## Single-Shot Over the Whole Set — Confirm MUST NOT Re-Invoke `submit_plan_node` (CONFIRM-03, D-17/D-19)

This is the critical soundness rule of this document, modeled directly on
`DESIGN-confirmation-release.md`'s `## Confirm MUST NOT Re-Invoke submit_plan_node` section.

**`caprun confirm` MUST NOT call `executor::submit_plan_node` a second time for the same
`effect_id`**, exactly as the v1.2 doc already requires. Taint is monotonic and is never cleared;
re-submitting the plan node would either re-Block (a no-op) or require a special-cased bypass inside
`submit_plan_node` itself — the exact "policy file disables I2" failure mode `CON-i2-non-bypassable`
forbids, now generalized to the whole blocked-arg set rather than one arg.

**MUST — confirm/deny is atomic over the WHOLE set:** `caprun confirm <effect_id>` MUST authorize the
WHOLE `(sink, {all blocked args}, combined_digest)` set atomically, or `caprun deny <effect_id>` MUST
deny ALL of it. There is NO partial-set confirm — there MUST NOT be a mechanism, CLI flag, or code
path that releases a subset of the blocked args (e.g., "confirm just the recipient, leave the body
pending") while leaving others pending or, worse, silently including the others unconfirmed in the
same re-invocation. This is the exact shape of the risk `DESIGN-content-adapter-mediation.md` exists
to close (D-02/D-12a) restated at the confirm-binding layer: a partial confirm would let a tainted
body ride an unrelated recipient's confirmation, exactly as an unstated single-arg Block precedence
once let a tainted body ride a recipient Block in the pre-collect-then-Block world.

**MUST — confirm/deny operates over the frozen snapshot only:** Confirm/deny MUST operate over the
frozen `resolved_args` (and frozen `combined_digest`) snapshot persisted at Block time. It MUST NOT
re-invoke `submit_plan_node`, MUST NOT re-resolve any `ValueId` against a live `ValueStore` (which
does not exist in the confirm process per "The Problem Being Solved" above), and MUST NOT recompute
`combined_digest` from anything other than reading the persisted value back.

---

## Done-When (Acceptance Predicate)

This document's design is satisfied when the following conditions ALL hold simultaneously:

1. **Combined digest over the full set is specified.** `caprun confirm` binds to ONE combined
   SHA-256 digest covering every blocked arg's resolved literal (recipient AND body together), never
   per-arg digests and never a subset, matching the set `DESIGN-content-adapter-mediation.md`'s
   collect-then-Block section defines (D-08, D-19).
2. **Post-transform mint rule is stated as a MUST.** The extractor mints `ValueRecord`s only AFTER
   any transformation, with no transform permitted between mint and Block or between Block and send
   — closing D-12(b) by construction rather than by a runtime check (CONFIRM-03, Pitfall 2).
3. **No-truncation display is stated as a MUST.** Every blocked arg's literal is shown verbatim, in
   full, for every arg in the set — no summary, no elision (CONFIRM-03, D-09).
4. **Every-arg block narration is stated as a MUST.** The Block moment shows provenance for every
   blocked arg individually, never a bare error and never only the first-matched arg (CONFIRM-04,
   D-20).
5. **Single-shot over the whole set is stated as a MUST, modeled on the existing "Confirm MUST NOT
   Re-Invoke `submit_plan_node`" pattern.** Confirm/deny is atomic over the WHOLE blocked-arg set —
   no partial confirm, no re-invocation of `submit_plan_node`, no re-resolution of any `ValueId`
   (CONFIRM-03, D-17/D-19).
6. **This document extends, not replaces, `PendingConfirmation`.** The combined digest is an
   additive field alongside the existing `resolved_args: Vec<ResolvedArg>`
   (`crates/brokerd/src/confirmation.rs`), following its existing "frozen at Block time, never
   re-derived" doc-comment convention (D-10).
7. **Cross-doc set agreement holds.** The combined-digest binding covers exactly the blocked-arg set
   `DESIGN-content-adapter-mediation.md`'s collect-then-Block section (D-14) produces — the two
   documents agree on the set shape, with no independent or divergent definition of "the blocked
   args" introduced here.

If any condition fails, this document is NOT ready for `DESIGN-GATE-RECORD-v1.3.md` to record
APPROVED/UNBLOCKED.
