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
of **every current `resolved_args` element** (blocked AND trusted, per the Round-6 amendment below —
not only the blocked subset), a binding shape that v1.2's single-arg-block world did not need
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

This document specifies the fix: `caprun confirm` binds to ONE combined SHA-256 digest over **every
element of the current `resolved_args`** (blocked AND trusted args together — an untainted `to` sitting
alongside a tainted `body` is bound too) — never a per-arg digest, and never a digest scoped to only
the blocked subset. **(Round-6 amendment, superseding the original blocked-subset-only scoping below —
see "Round-6: Full-Set Digest Domain" for the finding that forced this widening.)** "confirm", "the set
the human saw", and "the set the digest attests to" are, by construction, the same set: every arg the
sink will read. The send-side invariant is: **every arg the adapter sends — blocked or trusted — was in
the confirmed snapshot, byte-for-byte AND name-for-byte, and the digest attests to the exact set and its
exact element count**, so an arg silently added to (or removed from) `resolved_args` after Block time
changes the digest and fails closed, rather than passing unnoticed because the recompute only ever
looked at a fixed, pre-recorded name list.

---

## Combined-Digest Binding (CONFIRM-03, D-08/D-19)

**MUST:** `caprun confirm` binds to ONE combined SHA-256 digest covering the FULL SET of **every
current `resolved_args` element** — blocked AND trusted args together, e.g. recipient, subject, AND
body all bound as one set even when only body is tainted — never as separate per-arg digests, and
never scoped to only the blocked subset (Round-6 amendment; see below). The blocked-arg names recorded
per `DESIGN-content-adapter-mediation.md`'s "Collect-then-Block (D-14)" section remain meaningful as
DISPLAY-MARKING metadata (which args to narrate as BLOCKED vs TRUSTED) — they are no longer the
digest's domain-selecting filter.

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
keyed hash, no non-SHA-256 digest. The change from the existing single-arg pattern is the input, and it
MUST NOT be a plain literal concatenation.

**MUST (Round-6 amendment — bind names AND literals, over the FULL current set):** The combined digest
is SHA-256 over, for **every element currently in `resolved_args`** (not a subset filtered by any
pre-recorded name list), the concatenation `sha256(arg_name) ‖ sha256(literal)` (two 64-hex-char
fixed-width digests per element), taken in **byte-wise ascending order over the UTF-8 bytes of
`arg_name`** (Rust `str`'s `Ord` — not "ascending" unqualified, to remove any locale/collation
producer/verifier divergence risk). Binding the NAME alongside the literal (not literal-only) closes a
rename bypass: hashing literals alone in name-sorted order lets an actor rename a bound arg (e.g.
`body`→`cc`) without changing the sorted literal sequence at all, relying on the sink's own
required-arg validation (which happens to reject a missing `body`) as the only backstop — an
implementation detail, not a digest guarantee. Binding the name makes a rename change the digest
directly. Arg-name uniqueness within one `resolved_args` set MUST be asserted before hashing (already
available at Block time via `validate_schema`'s `DuplicateArg` check) — the ordering is undefined
otherwise.

**Why fixed-width per-element digests, not plain concatenation (closes the partition-blindness bypass,
finding #4, MUST):** Plain literal concatenation is PARTITION-BLIND: `H("a" ‖ "bc") == H("ab" ‖ "c")`,
so the digest cannot tell where one arg ends and the next begins. Concretely, a side-table write actor
that shifts the to/body boundary — `to="mallory@evil.co"` + `body="m sent…"` becoming
`to="mallory@evil.com"` + `body=" sent…"` — produces a BYTE-IDENTICAL concatenation, so a
recompute-and-compare over plain concatenation PASSES and mail goes to a recipient the human never
confirmed. This falsifies the "catches tampered `resolved_args`" claim. Hashing each arg's FIXED-WIDTH
(64-hex) `literal_sha256` in order removes it: every element occupies exactly 64 hex chars, so the
partition between args is fixed and any boundary shift changes at least one per-element digest and
therefore the combined digest.

**Non-normative aside (round-3 tightening — do NOT implement the raw form):** an earlier draft of this
section offered length-prefixing (`len(name) ‖ name ‖ len(literal) ‖ literal`) as an "equivalent"
alternative to a literal-only fixed-width scheme. Raw length-prefixed name+literal concatenation is NOT
equivalent to fixed-width per-element digests — it folds in RAW argument names and RAW literals rather
than fixed-width per-element digests, and a producer/verifier pair that picked different encodings
would silently diverge. The Round-6 `sha256(name) ‖ sha256(literal)` scheme above is NOT this retracted
form: both the name and the literal are independently hashed to a fixed 64-hex width BEFORE
concatenation, so there is no length-prefix ambiguity and no raw-byte encoding choice to diverge on.
This paragraph exists to distinguish the two, not to re-open the retraction.

This ALSO fixes the inversion round 2 noted — that a plain-concatenation combined
digest was WEAKER than the per-arg digests it summarizes: binding the partition makes the combined
digest genuinely attest to the exact per-arg boundaries, which plain concatenation did not.

**MUST (Round-6 amendment — the digest input set is every current `resolved_args` element, full stop):**
The digest input set is exactly the CURRENT contents of `PendingConfirmation.resolved_args` at
recompute time — blocked AND trusted args together — never a subset filtered by any pre-recorded name
list. **This supersedes the original scoping (a digest over only the collected `Vec<BlockedArg>`,
"a digest that also folds in trusted, untainted args... is a DIFFERENT set and MUST NOT be produced")**,
which a fresh adversarial round found insufficient: an actor with `pending_confirmations` write access
who APPENDS a new element to `resolved_args` after Block time (e.g. injecting a `bcc` the sink will
read) is invisible to a digest that only ever re-hashes a fixed, pre-recorded name list — the recompute
finds every name on its list unchanged and passes, while the sink reads and sends the injected element
too. Binding the FULL current set means an added, removed, or renamed element changes the element count
and/or a per-element digest, and the recompute-and-compare fails closed.

**MUST (verifier reproducibility over the full current set, not a recorded subset):** The digest is
exactly reproducible by an independent verifier recomputing, for every element in the CURRENT
`PendingConfirmation.resolved_args` (not a subset selected by recorded names), the
`sha256(arg_name) ‖ sha256(literal)` pair, sorted by byte-wise ascending `arg_name`, then hashed in that
order per the fixed-width per-element scheme above — NOT by filtering to a recorded blocked-arg-name
list, and NOT by plain-concatenating raw names or literals. The blocked arg-names recorded per
Collect-then-Block remain persisted as DISPLAY-MARKING metadata (which of the full set to narrate as
BLOCKED vs TRUSTED — see Block Narration below) but MUST NOT gate which elements enter the digest. A
producer hashing over the full current set while a verifier filters to a stale recorded subset — the
inverse of the set-mismatch bug the original scoping closed — is impossible when both recompute over
"every current `resolved_args` element," a property that needs no side-channel agreement on which names
to include.

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
    blocked_arg_names: Vec<String>,      // NEW — DISPLAY-MARKING ONLY (Round-6): which resolved_args
                                          // names to narrate as BLOCKED vs TRUSTED. Does NOT select
                                          // the digest's domain (that is now every resolved_args
                                          // element, blocked and trusted together).
    combined_digest: String,             // NEW — SHA-256 over EVERY current resolved_args element's
                                          // name+literal (Round-6), byte-wise-ascending-name order
                                          // (mirror of the value stored in the hashed sink_blocked
                                          // Event payload); frozen at Block time, recompute-and-compared
                                          // (never overwritten) at confirm/send, fail-closed on mismatch
    workspace_root_path: String,
    state:           PendingConfirmationState,
}
```

`combined_digest` MUST be computed and persisted ONCE, at the same Block-time write that persists
`resolved_args` (the same atomic transaction as the `sink_blocked` Event append, per
`DESIGN-confirmation-release.md`'s existing Persistence Contract). It MUST NOT be recomputed at
confirm time from a live re-resolution of any `ValueId` — recomputing from a live `ValueStore` would
reopen the exact in-memory-`ValueStore`-is-gone problem `DESIGN-confirmation-release.md`'s "The
Problem Being Solved" already ruled out for `resolved_args` itself.

**MUST (tamper-evidence — the digest lives inside the hashed anchor, and is recompute-and-compared):**
A digest that is written once and only ever read back verbatim adds no tamper-evidence — nothing ever
detects if the persisted row was altered. To make it load-bearing:

- **Persist `combined_digest` INSIDE the hashed `sink_blocked` EVENT payload** (the `Event` bytes that
  are hash-chained into the SHA-256 audit DAG) — not inside any individual `SinkBlockedAnchor` element.
  **(Round 5 reconciliation, phase-assignment-neutral):** this section originally said "inside the
  `SinkBlockedAnchor` payload," written before Phase 14 made `SinkBlockedAnchor` plural (one Event now
  carries a `Vec<SinkBlockedAnchor>`). The single digest that binds the WHOLE set (at Round-5
  authoring time: the blocked-arg subset; per the Round-6 amendment above, now every current
  `resolved_args` element) belongs at the Event level — one digest per Event, exactly where the whole
  set already lives — mirroring how Phase 15 added `derived_value_id`/`input_value_ids`/etc. to the
  derivation Event's hashed payload.
  This changes only the physical Rust field's location within the same hash-chained structure; the
  security property is identical: consistent with the project's existing audit-persistence decision, the
  DAG's own hash chain covers the digest, and any post-hoc edit to it (or to any anchor's frozen literal)
  breaks the chain. It is mirrored into `PendingConfirmation` for the confirm process to read, but the
  DAG Event copy is the tamper-evident source of truth.
- **The send runs in the confirm-path process; recompute-and-compare MUST precede it.** Since the
  confirmed send now runs in the SAME confirm-path process as the confirm decision (per
  `DESIGN-content-adapter-mediation.md`'s finding-#1 reversal — no daemon handoff), the code MUST,
  before performing the send, recompute the digest **from the frozen blocked-subset literals in the
  persisted snapshot** (NOT from any live `ValueId` re-resolution), per the fixed-width per-element
  scheme above, and compare it, byte-for-byte, to the `combined_digest` frozen in the `sink_blocked`
  Event. On ANY mismatch it MUST fail closed — refuse the send, `logger.error()` with context, and
  append a durable failure Event — NEVER proceed. This is what turns the digest from a write-only field
  into an actual integrity check: it catches a tampered `resolved_args` (literals changed without the
  digest) or a tampered digest (digest changed without the literals) before any irreversible send.
- **Same-snapshot MUST — no re-read between compare and send (closes a TOCTOU window, finding #5).**
  The frozen blocked-subset literals fed to the recompute-and-compare MUST be the SAME single in-memory
  read of the persisted snapshot that is then handed to the `lettre` message builder for the send. The
  code MUST NOT re-read the `pending_confirmations` / `resolved_args` row (or re-query the DB for the
  literals) between the compare and the send: read the frozen snapshot ONCE, compare against it, and
  build+send the `lettre` message from that same in-memory value. Re-reading the row after the compare
  would reopen a narrow window in which the persisted literals could differ between "what was compared"
  and "what is sent" — however unlikely given the CAS — so the compared bytes and the sent bytes MUST
  provably be one and the same in-memory snapshot.

The recompute-and-compare is over the FROZEN persisted literals, so it does NOT reintroduce the
live-`ValueStore` dependency the paragraph above rules out — "recompute from the frozen snapshot" and
"never re-resolve a `ValueId` live" are both true simultaneously.

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
Any transform MUST mint a FRESH `ValueRecord` for the transformed value BEFORE that value is ever used
as a plan-node arg or reaches the executor's per-arg loop.

---

## Provenance-Threading for Transform-Derived Mints (CONFIRM-03, D-16, MUST — closes the laundering BLOCKER)

The fresh-mint rule above is necessary but NOT sufficient. Copying *taint labels* onto a fresh mint
while re-anchoring its *lineage* is precisely the "taint stapled on at the sink proves nothing"
failure `CLAUDE.md` hard constraint #1 forbids — and it silently satisfies the letter of D-16
("an unbroken edge via `provenance_chain[0]` exists") because ANY fresh mint has *some* root. The
live `mint_from_read` always sets `provenance_chain = vec![fresh_event_id]` rooted at an event minted
in that same call, with no path to thread an input's provenance. A transform-derived value minted that
way would get a fresh chain rooted at a NEW event unrelated to the originating doc read — lineage
re-anchored, not propagated. The digest and the display would then attest to bytes whose audit-DAG
root does NOT descend from the untrusted read that made them dangerous. This section closes that.

**MUST (thread the provenance, do not re-anchor it):** A transform-derived mint's `provenance_chain`
MUST THREAD its inputs' chains: the derived value's provenance root MUST remain the originating
untrusted-doc read Event — OR the mint MUST record an explicit DAG derivation edge linking the
transform Event to EVERY input's read Event, and THAT edge is what D-16's per-blocked-arg unbroken-edge
assertion checks. Either way, tracing the derived arg's `provenance_chain` back MUST reach the
originating untrusted read(s); it MUST NOT terminate at a fresh event minted inside the transform with
no ancestry to the read.

**MUST (fail-closed on a re-anchored derived chain):** A transform-derived value whose
`provenance_chain` is a **fresh single-element chain** (rooted at a brand-new event with no threaded
ancestry to any input's read) MUST be a **fail-closed mint error** — the mint is rejected, exactly
mirroring the existing empty-`provenance_chain` guard pattern (a value with no provenance is already a
mint error today; a derived value with a *re-anchored* provenance is the same class of defect and MUST
be treated identically). The extractor MUST NOT produce such a value and hand it to the executor.

**MUST (the `mint_from_read` successor's provenance-threading contract, explicit):** The Phase-15
successor to `mint_from_read` (the transform/derive constructor) MUST take the input `ValueRecord`s
(or their `ValueId`s + resolved records) as arguments and construct the derived `ValueRecord` so that:
(1) `taint` is the union of the inputs' taint (monotonic, never narrowed); (2) `provenance_chain`
threads the inputs' chains such that its root is the originating read Event(s), never a fresh
transform-local root; (3) a durable transform/derivation Event is appended with parent edges to every
input's read Event, so the audit DAG carries the derivation as a real, hash-chained edge. A single-arg
signature that cannot receive the inputs' provenance (and would therefore be forced to fabricate a
fresh root) MUST NOT be the constructor used for a derived value.

**SHOULD (mechanical backstop — a future `check-invariants.sh` grep gate for the provenance root,
finding #6):** The "do not fabricate a fresh provenance root" rule above is currently doc-level
discipline. To give it a mechanical backstop mirroring this project's existing `EffectRequest`
token-ban gate (`check-invariants.sh` Gate 1), a future `scripts/check-invariants.sh` addition SHOULD
restrict the call sites of `mint_from_read` (and its Phase-15 transform/derive successor) to the
raw-read extraction module ONLY — the single module that legitimately roots a provenance chain at an
originating read Event — so any OTHER module attempting to mint a fresh-rooted `ValueRecord` fails the
build. The intended gate is a grep of the shape "the token `mint_from_read` (and its successor's name)
MUST NOT appear under `crates/` outside the raw-read extraction module," with the extraction module's
own definition/call site annotated as the sole allowed locus (the same allow-list-annotation pattern the
`EffectRequest` gate uses). **Phase 15 MUST implement this gate.** This was originally specified as a future, unscheduled
hardening item deferred past this documentation-only phase — but Phase 15 is the phase that
introduces derived/transform mints, making the laundering path this gate defends against load-bearing
for the first time. Deferring it further was correct only under the assumption that Phase 15 would
not widen the mint surface; that assumption is now false, so the gate lands in the same phase as the
surface it guards.

**MUST (Phase-15 fixture — twin to D-22's CRLF fixture):** Phase 15 MUST include a fixture that, for a
**transformed** tainted value (base64-decoded and/or concatenated), asserts the bytes shown at confirm
time AND covered by the `combined_digest` are **byte-identical** to the envelope Mailpit captures —
AND that the transformed arg's audit-DAG `provenance_chain` root traces back to the originating
untrusted-doc read Event, not to a fresh transform-local event. "Threaded by construction" MUST be
VERIFIED by this passing fixture, not assumed — the same discipline D-22 applies to the CRLF defense.

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

**MUST (Round-6: every sink-read arg, not only the blocked ones):** The Block moment narrates EVERY
arg the sink will read from `resolved_args` — not only the blocked subset — each explicitly marked
**BLOCKED** or **TRUSTED**. For a BLOCKED arg this MUST include: the sink and arg name, the literal
value verbatim, the taint labels, the source read Event id and session id, and the `provenance_chain`
summary. For a TRUSTED arg, at minimum the sink and arg name plus the literal value verbatim, so the
human sees the exact bytes the digest now binds for that arg too (Round-6 widened the digest domain to
the full set — the narration MUST widen in step, or the human confirms bytes the display never showed
them). This corrects `render_block_display`'s pre-Round-6 shape (a single-arg `assert!(blocked_count <=
1)` that PANICS on a genuinely-plural block, `crates/brokerd/src/confirmation.rs`) — the CONFIRM-04
rewrite replaces that assert with real per-arg iteration over the full set; per this project's standing
practice, prove the current panic fires (a committed regression test against the un-modified guard)
BEFORE replacing it, in a separate commit, so the guard's existing behavior is provably exercised, not
silently lost.

**MUST NOT:** The narration MUST NEVER be a bare `"Error: blocked"` with no per-arg detail, and MUST
NEVER show only the first-matched or only the blocked args while silently omitting trusted ones now
also bound by the digest. This is the human-legibility counterpart to collect-then-Block
(`DESIGN-content-adapter-mediation.md`'s D-14): if the collected set carries two tainted args (a tainted
`to` and a tainted `body`) alongside a trusted `subject`, the human MUST see all three literals and both
tainted args' provenance chains, side by side, before deciding to confirm or deny — never a display that
shows a subset and asks the human to trust that the confirm also covers "whatever else is in the set."

**MUST (ordering):** Per-arg narration MUST be presented in the same canonical order the combined
digest (above) was computed over, so the displayed order and the hashed order agree — a human
manually re-deriving the digest from the display (e.g., for an independent audit) MUST be able to do
so without needing to guess an ordering convention.

**MUST (Draft/untrusted-seeded posture is stated at confirm time, D-20 legibility):** The narration
MUST state the session's trust posture — that the session is `Draft` / untrusted-seeded (I0/I1) — and
that confirming authorizes an **irreversible EXTERNAL send** from that posture. A confirmed send
correctly bypasses Step 0.5's draft-only class-deny by construction (the per-arg I2 Block fires before
Step 0.5, and `caprun confirm` never re-runs `submit_plan_node` — see the Single-Shot section below),
so the human is not shielded by the class-deny here: their confirm IS the I0/I1 human gate for an
irreversible effect on a draft-only session. The narration MUST make that explicit — the human is told
they are releasing attacker-tainted bytes to an external recipient from a session that was seeded from
untrusted content — not leave it implicit behind a bare per-arg literal dump.

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
re-invoke `submit_plan_node`, and MUST NOT re-resolve any `ValueId` against a live `ValueStore` (which
does not exist in the confirm process per "The Problem Being Solved" above). It MUST, however,
recompute the digest **from every element of the frozen `resolved_args` snapshot (Round-6: the full
current set, not a recorded blocked-arg subset)** and compare it to
the `combined_digest` frozen in the hashed `sink_blocked` Event, failing closed on any mismatch (see
"tamper-evidence" under Combined-Digest Binding above). The distinction is exact: recomputing from the
FROZEN snapshot literals is REQUIRED (it is the integrity check); recomputing from a LIVE `ValueId`
re-resolution is FORBIDDEN (the `ValueStore` is gone). The stored digest is never overwritten — only
recomputed-for-comparison.

---

## Done-When (Acceptance Predicate)

This document's design is satisfied when the following conditions ALL hold simultaneously:

1. **Combined digest over the FULL current `resolved_args` set (Round-6) is specified, and it is
   PARTITION-BINDING and NAME-BINDING.** `caprun confirm` binds to ONE combined SHA-256 digest covering
   EVERY current `resolved_args` element's name and resolved literal (blocked AND trusted together,
   e.g. recipient, subject, AND body even when only body is tainted) — never per-arg digests, never
   scoped to only the blocked subset (superseded original scoping), and never a subset filtered by any
   pre-recorded name list. The digest is SHA-256 over, per element, `sha256(arg_name) ‖
   sha256(literal)` (both FIXED-WIDTH 64-hex), taken in byte-wise-ascending-`arg_name` order — NOT
   plain literal concatenation (partition-blind, admits the to/body boundary-shift bypass, finding #4)
   and NOT literal-only hashing (admits a rename bypass that relies on the sink's own required-arg
   validation as an accidental backstop). The verifier reproduces it by recomputing over every element
   CURRENTLY in `resolved_args` at recompute time, not a subset selected by recorded blocked-arg names —
   an arg appended, removed, or renamed after Block time changes the digest and fails closed.
2. **Post-transform mint rule AND provenance-threading are stated as MUSTs.** The extractor mints
   `ValueRecord`s only AFTER any transformation, with no transform permitted between mint and Block or
   between Block and send; AND a transform-derived mint's `provenance_chain` MUST THREAD its inputs'
   chains (root remains the originating untrusted read, or an explicit DAG derivation edge to every
   input's read), with a fresh single-element re-anchored chain on a derived value a fail-closed mint
   error — closing D-12(b) and the taint-laundering BLOCKER by construction rather than by a runtime
   check (CONFIRM-03, Pitfall 2, D-16).
2a. **Digest is tamper-evident, and compare + send read the same snapshot.** `combined_digest` is
   persisted inside the hashed `sink_blocked` Event payload (covered by the audit DAG hash chain) and
   is recompute-and-compared — from every element of the frozen `resolved_args` snapshot (Round-6: the
   full current set), never a live `ValueId` re-resolution — before the send, fail-closed on any
   mismatch. The send now runs in the confirm-path
   process (finding-#1 reversal, no daemon handoff), and the frozen literals fed to the compare MUST be
   the SAME single in-memory snapshot read handed to the `lettre` builder — no DB re-read between
   compare and send (finding #5) (CONFIRM-03, D-08).
2b. **The provenance-root rule has a specified future grep-gate backstop.** A future
   `check-invariants.sh` addition restricting `mint_from_read` (and its Phase-15 successor) call sites
   to the raw-read extraction module is specified as a DESIGN-level requirement (finding #6) — NOT
   implemented in this documentation-only phase (`scripts/check-invariants.sh` is untouched).
3. **No-truncation display is stated as a MUST.** Every blocked arg's literal is shown verbatim, in
   full, for every arg in the set — no summary, no elision (CONFIRM-03, D-09).
4. **Every-arg block narration is stated as a MUST, including the Draft posture.** The Block moment
   shows provenance for every blocked arg individually (never a bare error, never only the
   first-matched arg), AND states the session is `Draft`/untrusted-seeded and that confirming
   authorizes an irreversible external send from that posture — the confirm IS the I0/I1 human gate
   (CONFIRM-04, D-20).
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
