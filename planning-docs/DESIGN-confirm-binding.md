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

This document specifies the fix: `caprun confirm` binds to ONE combined SHA-256 digest over the
ordered **blocked-arg subset** (the collected `Vec<BlockedArg>`) — never a per-arg digest, and never
a digest over the FULL `resolved_args` (which also carries trusted, untainted args like an untainted
`to`). "confirm", "the set the human saw", and "the set the digest attests to" are, by construction,
the same set: exactly the blocked (tainted-sensitive) args. The send-side invariant is NOT "the
adapter sends only the blocked set" — the adapter of course sends the whole message, trusted args
included — it is: **every tainted-sensitive arg the adapter sends was in the confirmed blocked set,
byte-for-byte, and the digest attests to exactly that subset.**

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
keyed hash, no non-SHA-256 digest. The change from the existing single-arg pattern is the input, and it
MUST NOT be a plain literal concatenation. **The combined digest is SHA-256 over each blocked arg's
FIXED-WIDTH `literal_sha256` (the 64-hex-char per-element digest already computed per blocked arg — see
the single-arg `literal_sha256` pattern above and the per-element `Vec<BlockedArg>` anchor from
round-1's plural-anchor work), taken in blocked-arg order** — the ordered blocked-arg subset ONLY, NOT
the full `resolved_args`. The order is MUST-fixed and canonical (the order the collected
`Vec<BlockedArg>` was produced in by the per-arg loop, itself the stable `plan_node.args` iteration
order).

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

**Non-normative aside (round-3 tightening — do NOT implement this):** an earlier draft of this section
offered length-prefixing (`len(name) ‖ name ‖ len(literal) ‖ literal`) as an "equivalent" alternative.
It is NOT equivalent to the mandated per-arg-`literal_sha256` scheme — it folds in argument NAMES and
RAW literals rather than fixed-width per-element digests, and a producer/verifier pair that picked
different encodings would silently diverge. The per-arg `literal_sha256`-in-order scheme above is the
ONLY normative encoding; this paragraph exists solely to flag the prior wording as retracted, not to
offer a second valid option.

This ALSO fixes the inversion round 2 noted — that a plain-concatenation combined
digest was WEAKER than the per-arg digests it summarizes: binding the partition makes the combined
digest genuinely attest to the exact per-arg boundaries, which plain concatenation did not.

The digest input set is exactly the collected `Vec<BlockedArg>` (the tainted-sensitive args), full
stop; a digest that also folds in trusted, untainted args (e.g., an untainted `to`) is a DIFFERENT set
and MUST NOT be produced.

**MUST (verifier reproducibility over the subset, not the full arg set):** The digest is exactly
reproducible by an independent verifier recomputing each blocked arg's fixed-width `literal_sha256` over
the **blocked-arg subset filtered from `PendingConfirmation.resolved_args`** — selected by the recorded
blocked arg-names in the recorded order, then hashed in that order per the fixed-width per-element scheme
above — NOT by re-hashing raw `resolved_args` and NOT by plain-concatenating the literals. `PendingConfirmation` MUST therefore persist enough to
identify that subset deterministically (the ordered blocked arg-names, alongside the frozen
`resolved_args`), so producer and verifier hash the identical byte sequence. A producer hashing
`H(body)` while a verifier hashes `H(to‖body)` — the exact set-mismatch bug this finding closes —
is impossible when both are pinned to the recorded blocked-arg-name subset.

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
    blocked_arg_names: Vec<String>,      // NEW — ordered names of the blocked-arg SUBSET the digest
                                          // covers (selects the subset out of resolved_args)
    combined_digest: String,             // NEW — SHA-256 over the ordered BLOCKED-ARG SUBSET's literals
                                          // (mirror of the value stored in the hashed sink_blocked
                                          // anchor); frozen at Block time, recompute-and-compared
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

- **Persist `combined_digest` INSIDE the hashed `sink_blocked` anchor payload** (the `SinkBlockedAnchor`
  bytes that are hash-chained into the SHA-256 audit DAG), consistent with the project's existing
  audit-persistence decision — so the DAG's own hash chain covers the digest and any post-hoc edit to
  it (or to the anchor's frozen literals) breaks the chain. It is mirrored into `PendingConfirmation`
  for the confirm process to read, but the DAG anchor copy is the tamper-evident source of truth.
- **The send runs in the confirm-path process; recompute-and-compare MUST precede it.** Since the
  confirmed send now runs in the SAME confirm-path process as the confirm decision (per
  `DESIGN-content-adapter-mediation.md`'s finding-#1 reversal — no daemon handoff), the code MUST,
  before performing the send, recompute the digest **from the frozen blocked-subset literals in the
  persisted snapshot** (NOT from any live `ValueId` re-resolution), per the fixed-width per-element
  scheme above, and compare it, byte-for-byte, to the `combined_digest` frozen in the `sink_blocked`
  anchor. On ANY mismatch it MUST fail closed — refuse the send, `logger.error()` with context, and
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
`EffectRequest` gate uses). **This is a DESIGN-level specification of a FUTURE gate ONLY; this
documentation-only phase MUST NOT modify `scripts/check-invariants.sh` — adding the gate is a later
phase's action item.**

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
recompute the digest **from the frozen blocked-subset literals in that snapshot** and compare it to
the `combined_digest` frozen in the hashed `sink_blocked` anchor, failing closed on any mismatch (see
"tamper-evidence" under Combined-Digest Binding above). The distinction is exact: recomputing from the
FROZEN snapshot literals is REQUIRED (it is the integrity check); recomputing from a LIVE `ValueId`
re-resolution is FORBIDDEN (the `ValueStore` is gone). The stored digest is never overwritten — only
recomputed-for-comparison.

---

## Done-When (Acceptance Predicate)

This document's design is satisfied when the following conditions ALL hold simultaneously:

1. **Combined digest over the blocked-arg subset is specified, and it is PARTITION-BINDING.** `caprun
   confirm` binds to ONE combined SHA-256 digest covering every BLOCKED arg's resolved literal
   (recipient AND body together) — the ordered blocked-arg subset ONLY, never per-arg digests and never
   the full `resolved_args` (which also carries trusted args). The digest is SHA-256 over each blocked
   arg's FIXED-WIDTH (64-hex) `literal_sha256` in blocked-arg order — NOT plain literal concatenation,
   which is partition-blind and admits the to/body
   boundary-shift bypass (finding #4). The verifier reproduces it by recomputing each blocked arg's
   `literal_sha256` over that subset filtered from `resolved_args` by recorded arg-name+order, matching
   the set `DESIGN-content-adapter-mediation.md`'s collect-then-Block section defines (D-08, D-19).
2. **Post-transform mint rule AND provenance-threading are stated as MUSTs.** The extractor mints
   `ValueRecord`s only AFTER any transformation, with no transform permitted between mint and Block or
   between Block and send; AND a transform-derived mint's `provenance_chain` MUST THREAD its inputs'
   chains (root remains the originating untrusted read, or an explicit DAG derivation edge to every
   input's read), with a fresh single-element re-anchored chain on a derived value a fail-closed mint
   error — closing D-12(b) and the taint-laundering BLOCKER by construction rather than by a runtime
   check (CONFIRM-03, Pitfall 2, D-16).
2a. **Digest is tamper-evident, and compare + send read the same snapshot.** `combined_digest` is
   persisted inside the hashed `sink_blocked` anchor payload (covered by the audit DAG hash chain) and
   is recompute-and-compared — from the frozen blocked-subset literals, never a live `ValueId`
   re-resolution — before the send, fail-closed on any mismatch. The send now runs in the confirm-path
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
