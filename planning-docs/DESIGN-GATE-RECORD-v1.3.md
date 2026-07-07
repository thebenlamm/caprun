# DESIGN Gate Record — v1.3

**Date:** 2026-07-07
**Reviewer (round 1):** Fresh-context adversarial panel (3 reviewers + Fable) arranged by
caprun-opus-77 (D-11; NOT caprun-sonnet-77, the session that authored
`DESIGN-content-adapter-mediation.md` and `DESIGN-confirm-binding.md`), independently verified by
opus. Authorized under `DEC-ai-review-satisfies-human-gate` (Ben Lamm, same precedent as v1.2).
**Reviewer (round 2):** TBD — a fresh round-2 pass by the same external panel, still pending.
**Phase:** 12-content-adapter-confirm-binding-design-gate — Plan 03
**Review round:** 1 COMPLETE — Decision: NEEDS REVISION (8 findings, all fixes applied); round 2 pending.

## Revision History

- **Round 1 — NEEDS REVISION** (fresh-context adversarial panel — 3 reviewers + Fable — arranged by
  caprun-opus-77 under `DEC-ai-review-satisfies-human-gate`, independently verified by opus; NOT the
  authoring session, per D-11). The panel found **1 BLOCKER, 2 MAJOR, 2 GAP, 1 MUST-RESOLVE, 1
  UNDERSPECIFIED, 1 SHOULD-FIX** (8 findings). This is the exact "gate earns its cost" outcome the
  process is designed to produce — mirroring v1.2's own round-1 B1 blocker. Opus issued an explicit
  resolution mandate for each finding ("implement it unless you have a concrete objection"); there was
  no objection, and all 8 were implemented as specified.

  **D-21 re-verification: CONFIRMED.** A reviewer independently read
  `crates/executor/src/sink_sensitivity.rs:93-98` directly and confirmed `is_content_sensitive`
  returns `true` for `email.send`'s `subject`/`body`/`attachment` — the D-21 claim is verified against
  live source, not accepted on the DESIGN/research doc's word.

  **The 8 findings, their severity, and the mandated resolution applied:**

  1. **BLOCKER — transform launders provenance (the project's #1 invariant).**
     `DESIGN-confirm-binding.md` minted a fresh `ValueRecord` for an EXTRACT-03 transform "inheriting
     taint from its inputs" but was silent on `provenance_chain`; the live `mint_from_read` always
     roots the chain at a fresh event minted in the same call, so a transformed value would get a
     chain re-anchored to a NEW read event, not descending from the original doc read — copied taint
     with re-anchored lineage = stapled taint (`CLAUDE.md` hard constraint #1). *Resolution:* both docs
     now MUST that a transform-derived mint's `provenance_chain` THREADS its inputs' chains (root stays
     the originating untrusted read, or an explicit DAG derivation edge to every input's read, and THAT
     edge is what D-16 asserts per blocked arg); a fresh single-element chain on a derived value is a
     fail-closed mint error; the `mint_from_read`-successor threading contract is specified; a Phase-15
     byte-identical-at-Mailpit fixture (twin to D-22) is mandated. (`DESIGN-confirm-binding.md` new
     "Provenance-Threading for Transform-Derived Mints" section; `DESIGN-content-adapter-mediation.md`
     D-16 strengthened. Commit `92c6487` + `5dc1e67`.)
  2. **MAJOR — where does the confirmed send run?** The inherited v1.2 `file.create` pattern invokes
     the sink in the standalone `caprun confirm` CLI, which for `email.send` would put the SMTP call —
     and secrets — in the confirm process, violating D-04. *Resolution:* the confirmed SMTP send MUST
     run in the BROKER DAEMON; `caprun confirm` hands the confirmed `effect_id` to the running broker
     over UDS and never holds a secret. (`DESIGN-content-adapter-mediation.md` Adapter Mediation
     Boundary, D-03 refinement. Commit `5dc1e67`.)
  3. **MAJOR — at-most-once across the CLI/broker split + no error swallowing.** The split reopens
     double-fire/silent-loss, and the inherited `Err(_) => Ok(ConfirmedButSinkFailed)` swallows the
     send error. *Resolution:* durable idempotency token per `effect_id`; `email_send_attempted` Event
     before any wire action + `email_send_failed` on error; explicit no-auto-retry recovery rule for a
     confirmed-but-unsent irreversible send; never-swallow / never-`unwrap` error path (SEND-01/SEND-02,
     D-24). (`DESIGN-content-adapter-mediation.md` new "At-Most-Once Send" section. Commit `5dc1e67`.)
  4. **GAP — the combined digest bound an ambiguous set.** One section said "every BLOCKED arg's
     literal"; another said "re-hashing `PendingConfirmation.resolved_args`" (the FULL set) — producer
     and verifier could hash different sets. *Resolution:* the digest input is the ordered BLOCKED-ARG
     SUBSET (the collected `Vec<BlockedArg>`), full stop; the verifier re-hashes that subset filtered
     from `resolved_args` by recorded arg-name+order; dropped the false "adapter sends only the blocked
     set" clause, replaced with "every tainted-sensitive arg the adapter sends was in the confirmed
     blocked set." (`DESIGN-confirm-binding.md` intro + Combined-Digest Binding. Commit `92c6487`.)
  5. **MUST-RESOLVE — `combined_digest` was write-only (never recomputed/compared).** *Resolution:*
     persist `combined_digest` INSIDE the hashed `sink_blocked` anchor payload (covered by the audit
     DAG hash chain); confirm AND send MUST recompute-and-compare it against the frozen blocked-subset
     literals before releasing, fail-closed on mismatch — recompute from the FROZEN snapshot (required),
     never a LIVE `ValueId` re-resolution (forbidden). (`DESIGN-confirm-binding.md` Combined-Digest
     Binding + Single-Shot section. Commit `92c6487`.)
  6. **GAP — the `attachment` arg was unhandled.** `attachment` is schema-accepted AND content-sensitive
     (blocks), so a tainted attachment would be Blocked→confirmed→must-send, but SMTP-05's typed-builder
     allow-list has no attachment path and the Content-Disposition filename→header CRLF surface is
     unanalyzed. *Resolution (start-simplest):* DESCOPE `attachment` for v1.3 — remove it from
     `email.send`'s schema `allowed` set AND from `EMAIL_SEND_CONTENT_SENSITIVE`; `subject`/`body` stay
     (analyzed, safe). (`DESIGN-content-adapter-mediation.md` new "Attachment Is Descoped" D-23 section;
     D-21 classification section updated. Commit `5dc1e67`.)
  7. **UNDERSPECIFIED — lettre rejection semantics.** The doc mandated lettre rejects CRLF-in-address at
     parse time but never said what the adapter does with that `Err` on a confirmed literal.
     *Resolution:* any lettre construction `Err` on a confirmed literal → fail-closed AUDITED abort
     (durable failure Event), never panic, never silent drop (same discipline as #3).
     (`DESIGN-content-adapter-mediation.md` Wire-Message Construction, D-07 refinement. Commit `5dc1e67`.)
  8. **SHOULD-FIX (legibility) — fold into D-20.** A confirmed send correctly bypasses Step 0.5's
     draft-only class-deny by construction, but the per-arg narration should also state the session is
     Draft/untrusted-seeded and that confirming authorizes an irreversible external send from that
     posture. *Resolution:* added that MUST to the Block Narration section. (`DESIGN-confirm-binding.md`
     Block Narration, D-20. Commit `92c6487`.)

- **Round 2 — pending.** The revised docs (re-hashed below) await a fresh round-2 adversarial pass by
  caprun-opus-77's external panel. Decision remains **NEEDS REVISION** and Gate status remains
  **BLOCKED** until that round-2 review signs off — this authoring session MUST NOT self-approve (D-11).

## Documents Under Review

These are the **post-round-1-fix (round-2 input)** hashes. Both docs were revised per the 8 round-1
findings above; the stale round-1 hashes (`c2506396…` / `c7a61423…`) no longer match and are retained
only in this note for provenance.

| Document | sha256 (round-2 input, post-fix) |
|----------|--------|
| `planning-docs/DESIGN-content-adapter-mediation.md` | `bec703fef52a6342a38d2924ef4f56b0b18c6873c09388bd8a2928fa630ec07e` |
| `planning-docs/DESIGN-confirm-binding.md` | `68dfd9d9e8c6c4e538234c5b0130914fbf77be9a0c65f6c9509292a8c54eb470` |

Hashes were re-computed with `shasum -a 256` after the round-1 fixes. The round-2 reviewer MUST
re-run `shasum -a 256 planning-docs/DESIGN-content-adapter-mediation.md planning-docs/DESIGN-confirm-binding.md`
and confirm the values match before setting Decision: APPROVED. If the DESIGN docs are amended again
during a further fix→re-review loop, re-hash them here and note the round.

<!-- shasum -->
```
$ shasum -a 256 planning-docs/DESIGN-content-adapter-mediation.md planning-docs/DESIGN-confirm-binding.md
bec703fef52a6342a38d2924ef4f56b0b18c6873c09388bd8a2928fa630ec07e  planning-docs/DESIGN-content-adapter-mediation.md
68dfd9d9e8c6c4e538234c5b0130914fbf77be9a0c65f6c9509292a8c54eb470  planning-docs/DESIGN-confirm-binding.md
```
(Re-run after round-1 fixes, 2026-07-07; commits `5dc1e67` + `92c6487`. Prior round-1 hashes:
`c2506396852d4bd619d7985cf2973cdd3b140177cff3c5d82f53038b3fa6724c` /
`c7a614233324f8a3d012a27836e4b891f27f2aff4197bcbd8d85e3db65b3f1f2`.)

---

## Checklist

Each item maps one-to-one to a CONTENT/SMTP/CONFIRM requirement or a D-xx decision in
`.planning/phases/12-content-adapter-confirm-binding-design-gate/12-CONTEXT.md`. Boxes are
pre-filled by grep: a box is checked only if the corresponding grep matched the target document.
Unchecked items indicate missing required content — the doc(s) must be revised before approval.

### DESIGN-content-adapter-mediation.md

- [x] **Item 1 — Content-sensitivity classification scope: single hardcoded match arm, not a
  general taxonomy** (CONTENT-01/CONTENT-02, D-01)
  - Grep matched: `grep -c 'single hardcoded match arm'` → 1; `grep -ci 'CONTENT-02'` → 2;
    `grep -ci 'MUST NOT be generalized\|MUST NOT grow'` → 2.

- [x] **Item 2 — `is_content_sensitive` already exists; Phase 14's work is Step 3's consequence,
  not new classification** (D-21)
  - Grep matched: `grep -c 'is_content_sensitive'` → 2 (cited existing function, `sink_sensitivity.rs:93-98`);
    `grep -ci 'already exists\|already returns\|do not re-implement\|MUST NOT duplicate'` → 3;
    `grep -c 'Independent re-verification required'` → 1.
  - **Round-1: D-21 CONFIRMED** by a panel reviewer reading `sink_sensitivity.rs:93-98` directly
    (`is_content_sensitive` returns `true` for `subject`/`body`/`attachment`). Round-1 finding #6 then
    DESCOPED `attachment` for v1.3 (D-23) — Phase 13/14 must remove it from `EMAIL_SEND_CONTENT_SENSITIVE`
    and the schema `allowed` set; live content set becomes `subject`/`body`. (Commit `5dc1e67`.)

- [x] **Item 3 — Precedence between routing-block and body-block is explicit** (CONTENT-01, D-02)
  - Grep matched: `grep -ci 'precedence'` → 6; `grep -c 'Precedence — Routing-Block vs Body-Block'` → 1.

- [x] **Item 4 — Collect-then-Block: per-arg loop collects all sensitive+tainted args before
  any Block; decision/anchor types become plural** (D-14)
  - Grep matched: `grep -c 'Collect-then-Block'` → 2; `grep -ci 'plural'` → 2;
    `grep -c 'Vec<BlockedArg>'` → 2.

- [x] **Item 5 — Worker-never-sends; SMTP secrets live only in the broker** (SMTP-01/02, D-03/D-04)
  - Grep matched: `grep -c 'Worker-never-sends'` → 1; `grep -c 'Secrets broker-only'` → 1;
    `grep -c 'crates/brokerd/src/sinks/email_smtp.rs'` → 2.
  - **Round-1 (findings #2, #3):** added the D-03 refinement — the confirmed send runs in the BROKER
    DAEMON, not the `caprun confirm` CLI (which hands `effect_id` over UDS and holds no secret) — and a
    new "At-Most-Once Send" section (durable `email_send_attempted`/`_failed` ledger, no auto-retry,
    never-swallow error path; SEND-01/SEND-02, D-24). (Commit `5dc1e67`.)

- [x] **Item 6 — Kernel-enforced negative net assertion against the existing seccomp mechanism**
  (SMTP-01, D-05)
  - Grep matched: `grep -c 'seccomp'` → 4; `grep -c 'apply_worker_filter'` → 1;
    `grep -ci 'negative net\|negative_net'` → 3.

- [x] **Item 7 — Local capture SMTP target (Mailpit); live SES explicitly out of gate scope**
  (SMTP-03, D-06)
  - Grep matched: `grep -ci 'mailpit'` → 6; `grep -ci 'live SES is OUT of gate scope\|out of gate scope'` → 1.

- [x] **Item 8 — CRLF/header-injection defense mechanics specified; forbidden constructs named;
  Phase 13 fixture mandated** (SMTP-05, D-07/D-22)
  - Grep matched: `grep -c 'dangerous_new_pre_encoded'` → 2; `grep -ci 'CRLF'` → 5;
    `grep -c 'boring-tls'` → 2; `grep -ci 'fixture'` → 2.
  - **Round-1 (finding #7):** added lettre rejection semantics — any lettre construction `Err` on a
    confirmed literal is a fail-closed AUDITED abort (durable failure Event, never panic/silent-drop,
    D-07 refinement). The unanalyzed `attachment` Content-Disposition CRLF surface is removed from
    scope by finding #6's descope (D-23). (Commit `5dc1e67`.)

- [x] **Item 9 — Plan-node API confirmed untouched; no `EffectRequest` path introduced** (D-18)
  - Grep matched: `grep -c 'Plan-Node API Is Untouched'` → 1; `grep -c 'EffectRequest'` → 1
    (mentioned only to state it is NOT introduced — annotate per `check-invariants.sh` Gate 1
    if this trips the repo-wide token gate).

### DESIGN-confirm-binding.md

- [x] **Item 10 — Combined SHA-256 digest over the blocked-arg SUBSET of blocked args' literals,
  never per-arg and never the full `resolved_args`** (CONFIRM-03, D-08/D-19)
  - Grep matched: `grep -c 'Combined-Digest Binding'` → 1; `grep -ci 'combined_digest'` → 8+;
    `grep -ci 'never per-arg\|never a per-arg digest'` → 1.
  - **Round-1 (findings #4, #5):** the digest set was disambiguated to the ordered BLOCKED-ARG SUBSET
    ONLY (verifier re-hashes that subset filtered from `resolved_args` by recorded arg-name+order, not
    raw `resolved_args`), and `combined_digest` was made tamper-evident — persisted inside the hashed
    `sink_blocked` anchor and recompute-and-compared (fail-closed) from the frozen snapshot at
    confirm/send. (Commit `92c6487`.) NOTE: the item title's original "FULL SET" wording is superseded
    by "blocked-arg SUBSET" per finding #4.

- [x] **Item 11 — Post-transformation-bytes rule: mint after transform, no drift between
  confirm and send; provenance threaded** (CONFIRM-03, D-08, D-16, Pitfall 2)
  - Grep matched: `grep -c 'Post-Transformation Bytes'` → 1; `grep -ci 'mint-after-transform\|mint only AFTER'` → 2;
    `grep -c 'Why this closes D-12(b)'` → 1; `grep -c 'Provenance-Threading for Transform-Derived Mints'` → 1.
  - **Round-1 (BLOCKER #1):** added the provenance-threading contract — a transform-derived mint's
    `provenance_chain` MUST thread its inputs' chains (root stays the originating untrusted read); a
    fresh single-element re-anchored chain is a fail-closed mint error; the `mint_from_read`-successor
    contract + a Phase-15 byte-identical-at-Mailpit fixture are specified. (Commit `92c6487`.)

- [x] **Item 12 — No-truncation verbatim display for every blocked arg** (CONFIRM-03, D-09)
  - Grep matched: `grep -c 'Verbatim Display — No Truncation'` → 1; `grep -ci 'no truncation\|MUST NOT truncate'` → 2.

- [x] **Item 13 — Block narration covers every blocked arg individually, in canonical order
  matching the digest, and states the Draft posture** (CONFIRM-04, D-20)
  - Grep matched: `grep -c 'Block Narration for Every Arg'` → 1; `grep -ci 'never only the first-matched arg'` → 1.
  - **Round-1 (SHOULD-FIX #8):** narration MUST now also state the session is `Draft`/untrusted-seeded
    and that confirming authorizes an irreversible external send from that posture (the confirm is the
    I0/I1 human gate). (Commit `92c6487`.)

- [x] **Item 14 — Single-shot over the whole set; confirm MUST NOT re-invoke `submit_plan_node`**
  (CONFIRM-03, D-17/D-19)
  - Grep matched: `grep -c 'MUST NOT.*submit_plan_node'` → 2; `grep -ci 'atomic over the WHOLE set\|NO partial-set confirm'` → 2.

- [x] **Item 15 — Extends, does not replace, `PendingConfirmation`/`ResolvedArg`** (D-10)
  - Grep matched: `grep -c 'PendingConfirmation'` → 4; `grep -ci 'additive'` → 3.

### Collect-then-Block precedence + D-21 re-verification (both documents)

- [x] **Item 16 — Collect-then-Block precedence (D-02/D-14) is stated as a MUST in both docs and
  the two documents agree on the blocked-arg set shape**
  - Grep matched: `grep -c 'Cross-doc set agreement'` → 1 in `DESIGN-confirm-binding.md`;
    `grep -ci 'the two documents MUST agree on the set shape'` → 1 in `DESIGN-content-adapter-mediation.md`.

- [x] **Item 17 — D-21 re-verification obligation is stated explicitly as a reviewer MUST, not
  merely asserted by the authoring document**
  - Grep matched: `grep -c 'Independent re-verification required'` → 1 in
    `DESIGN-content-adapter-mediation.md`; `grep -c 'sink_sensitivity.rs:93-98'` → 2 — confirms the
    doc names the exact file/line the reviewer must independently re-read, rather than asking the
    reviewer to trust the doc's own citation.

---

## Both Documents — Soundness

Completeness greps pass fully-written-but-wrong specs; this section gates *soundness*, not
presence — mirroring the section that caught B1 in `DESIGN-GATE-RECORD-v1.2.md`. Each item below
is a directed adversarial re-read requiring the reviewer to trace the property to an actual
section + file/line in the DESIGN docs, NOT a keyword count. This is the core work of Task 2 and
is intentionally left as an open finding-slot for the fresh-context reviewer, not pre-filled by
this authoring session.

- [ ] **Item 18 — D-12(a): Can CONTENT-01's body-block and the routing-block compose into an
  unconfirmable dead end (the B1 failure mode reincarnated for the body arg)?**
  - `DESIGN-content-adapter-mediation.md`'s "Precedence — Routing-Block vs Body-Block" and
    "Collect-then-Block (D-14)" sections claim the per-arg loop collects ALL sensitive+tainted
    args (routing OR content) in one pass before any Block is returned, and that both surface as
    one combined `BlockedPendingConfirmation`, confirmable/deniable as one set via
    `DESIGN-confirm-binding.md`'s combined-digest mechanism (D-17/D-19).
  - **Reviewer MUST independently verify:** (1) the collect-all-then-Block loop shape actually
    makes BOTH args confirmable in one decision, not merely stated to; (2) Step 0.5's I2-over-I1
    precedence (D-15) is genuinely preserved — i.e., the collect-all loop still completes with NO
    Block before Step 0.5 runs, exactly as today, and this reordering does not itself open a new
    variant of B1; (3) the "Illustrative shape" Rust snippet in "Collect-then-Block" is consistent
    with the prose MUST statements, not merely decorative.
  - **Round-1 finding (BLOCKER, finding #1) + fix:** The panel found the *composition* held but the
    provenance path did NOT — a transform-derived (EXTRACT-03) blocked arg could get a fresh
    `provenance_chain` re-anchored to a new read event, laundering lineage while satisfying D-16's
    letter. Fixed: D-16 strengthened + a new provenance-threading contract added
    (`DESIGN-confirm-binding.md`); a re-anchored derived chain is now a fail-closed mint error, and a
    Phase-15 fixture verifies byte-identity + originating-read root. Also the single-shot atomicity
    that makes both args confirmable in one decision was made tamper-evident (finding #5).
  - **Status:** OPEN — round-1 fix applied (commits `92c6487`, `5dc1e67`); AWAITING round-2
    confirmation. NOT resolved by the authoring session (D-11).

- [ ] **Item 19 — D-12(b): Can CONFIRM-03's literal-binding hash be computed over pre-transformation
  bytes instead of the post-EXTRACT-03-transformation bytes actually sent?**
  - `DESIGN-confirm-binding.md`'s "Post-Transformation Bytes — No Drift Between Confirm and Send"
    section claims the mint-after-transform + no-transform-after-mint MUSTs close this by
    construction (no runtime check needed) because the combined digest is computed over the frozen
    `ResolvedArg.literal`, which is guaranteed to already be the exact post-transform bytes.
  - **Reviewer MUST independently verify:** (1) the "MUST NOT — the specific anti-pattern this
    rules out" paragraph actually forecloses a Phase-15 extractor resolving-then-transforming a
    `ValueId` without minting a fresh `ValueRecord`; (2) there is no path, named or unnamed, in
    either document where a transform could run between mint and Block, or between Block and
    adapter invocation; (3) the combined-digest computation (`combined_digest` field,
    `crates/brokerd/src/confirmation.rs:100-124` extension) is genuinely frozen at Block time — the
    doc's own MUST statements must be checked against the schema, not merely quoted.
  - **Round-1 findings (BLOCKER #1, GAP #4, MUST-RESOLVE #5) + fixes:** the panel confirmed this is
    the vector where the fresh-mint rule was necessary-but-not-sufficient (provenance re-anchoring,
    #1), where the digest bound an ambiguous set (full `resolved_args` vs blocked subset, #4), and
    where `combined_digest` was write-only with no integrity check (#5). All three fixed: digest now
    covers the ordered BLOCKED-ARG SUBSET only (verifier re-hashes that subset); `combined_digest`
    persisted inside the hashed `sink_blocked` anchor and recompute-and-compared (fail-closed) from
    the frozen snapshot at confirm/send — required, while a LIVE `ValueId` re-resolution stays
    forbidden; provenance threaded to the originating read. (Commits `92c6487`, `5dc1e67`.)
  - **Status:** OPEN — round-1 fix applied; AWAITING round-2 confirmation. NOT resolved by the
    authoring session (D-11).

- [ ] **Item 20 — D-12(c): Does SMTP-05's message construction have any path where a tainted
  literal reaches a header?**
  - `DESIGN-content-adapter-mediation.md`'s "Wire-Message Construction — CRLF/Header-Injection
    Defense" section claims the typed-builder-only requirement (`lettre >= 0.11.22`,
    `Message::builder()`), the forbidden-constructs list (`dangerous_new_pre_encoded`, `format!`-built
    headers), and the "body is written after the blank-line separator" structural argument together
    close this vector, backed by direct citation of `lettre`'s `Address::new` and
    `HeaderValueEncoder::allowed_char` source behavior.
  - **Reviewer MUST independently verify:** (1) the cited `lettre` source behavior
    (`Address::new`'s allow-list grammar excluding CR/LF; `HeaderValueEncoder::allowed_char`
    excluding bytes 10/13) is accurate for the pinned version (`>= 0.11.22`), not merely asserted;
    (2) the "why the body is safe" argument is a genuine structural (call-boundary) guarantee — i.e.,
    that the adapter code path described truly never concatenates the body literal into any
    header-construction call — and not merely an RFC 5322 parsing argument that happens to be true
    of well-behaved MTAs but unenforced by the adapter's own code shape; (3) the Phase-13 CRLF
    fixture requirement (D-22) is specific enough to actually falsify a broken implementation (i.e.,
    it asserts on Mailpit's captured envelope recipients, not merely "send succeeded").
  - **Round-1 findings (GAP #6, UNDERSPECIFIED #7) + fixes:** the panel found the `attachment` arg
    unhandled (schema-accepted + content-sensitive, but no typed-builder path and an unanalyzed
    Content-Disposition CRLF surface) and the lettre `Err`-on-confirmed-literal semantics
    unspecified. Fixed: `attachment` DESCOPED for v1.3 (removed from the schema `allowed` set and
    `EMAIL_SEND_CONTENT_SENSITIVE`, D-23), so the header/CRLF surface it introduced is out of scope
    by construction; a lettre construction `Err` on a confirmed literal is now a fail-closed AUDITED
    abort (durable failure Event, never panic/silent-drop, D-07 refinement). (Commit `5dc1e67`.)
  - **Status:** OPEN — round-1 fix applied; AWAITING round-2 confirmation. NOT resolved by the
    authoring session (D-11).

MUST/MUST NOT density: `grep -c 'MUST'` → run at review time on both files; expected to be
comparable to the v1.2 analog docs' density (40s-70s range per file) given both documents' MUST-
heavy structure — reviewer should confirm this holds, not merely assume it from this note.

---

## How to Verify (Human Review Steps)

Before setting Decision and Gate status, the reviewer MUST:

1. **Confirm all seventeen completeness checklist items (1-17) are checked.** If any box is
   unchecked, the corresponding doc is incomplete — it must be revised before approval.

2. **Perform the three D-12 soundness re-reads (Items 18-20) as an attacker, not a proofreader.**
   For each of D-12(a), D-12(b), D-12(c): trace the closing argument to an actual section + file/line
   in the DESIGN docs (and, where cited, the actual current source — `crates/executor/src/lib.rs`,
   `crates/runtime-core/src/executor_decision.rs`, `crates/brokerd/src/confirmation.rs`,
   `crates/executor/src/sink_sensitivity.rs`) — not merely confirm the keyword is present. Record
   each finding by severity (BLOCKER/MAJOR/minor) using the `DESIGN-REVIEW-v1.2-round1.md` format
   if any gap is found; a round-1 BLOCKER is an expected, successful outcome (as v1.2's B1 was), not
   a process failure.

3. **Independently re-verify the D-21 claim** that `is_content_sensitive`
   (`crates/executor/src/sink_sensitivity.rs:93-98`, currently lines ~60-67 per direct read at
   authoring time — reviewer MUST re-locate against current `main`) already returns `true` for
   `email.send`'s `subject`/`body`/`attachment` args, and that Phase 14's real work is changing Step
   3's consequence in `crates/executor/src/lib.rs` (no-op → Block), not adding new classification
   code. Do not accept this on the DESIGN doc's or the research doc's word alone (D-21 is explicit
   on this point).

4. **Confirm the two sha256 hashes match the current files:** run
   `shasum -a 256 planning-docs/DESIGN-content-adapter-mediation.md planning-docs/DESIGN-confirm-binding.md`
   and compare the output to the values in the "Documents Under Review" table above (re-run again
   after any fix→re-review cycle and re-hash the table).

5. **Confirm cross-document agreement:** `DESIGN-confirm-binding.md`'s combined-digest set MUST
   cover exactly the blocked-arg set `DESIGN-content-adapter-mediation.md`'s collect-then-Block
   section produces — no independent or divergent definition of "the blocked args" between the two
   documents.

6. **If a BLOCKER/MAJOR is found:** record the finding in this record using the v1.2 round-1
   severity-prefixed format (BLOCKER/MAJOR/minor + "What's right" + "Suggested resolution order"),
   request the DESIGN doc(s) be fixed, re-hash them above, and repeat this fix→re-review loop until
   no blocker/major remains.

7. **If satisfied (no unresolved blocker/major, all D-12 vectors traced to file/line, D-21
   independently re-verified):** set Decision to APPROVED and Gate status to UNBLOCKED (below),
   dated, naming the fresh-context reviewer and the `DEC-ai-review-satisfies-human-gate`
   authorization if applicable.
   **If not satisfied:** set Decision to NEEDS REVISION, list the gaps, and the phase loops again.

---

## Decision

**Decision:** NEEDS REVISION

**Round 1 is COMPLETE and returned NEEDS REVISION** (8 findings — 1 BLOCKER, 2 MAJOR, 2 GAP, 1
MUST-RESOLVE, 1 UNDERSPECIFIED, 1 SHOULD-FIX — see Revision History above), by a fresh-context
adversarial panel (3 reviewers + Fable) arranged by caprun-opus-77 under
`DEC-ai-review-satisfies-human-gate`, independently verified by opus, NOT the authoring session
(D-11). **Round 1 fixes applied (commits: `5dc1e67` [content-adapter-mediation.md, findings
#2/#3/#6/#7], `92c6487` [confirm-binding.md, findings #1/#4/#5/#8]).** Both docs re-hashed in the
Documents Under Review table above (round-2 input hashes).

**Awaiting round-2 review by caprun-opus-77's fresh-context panel.** Decision remains NEEDS REVISION
and MUST NOT be set to APPROVED by this authoring session — only a fresh round-2 adversarial pass by
the same external panel (confirming the 8 fixes hold and open no new defect, re-verifying D-21, and
re-running `shasum -a 256` against the round-2 hashes) may set Decision: APPROVED / Gate status:
UNBLOCKED. Per D-11 and the recorded v1.2 revert-when-self-reviewed precedent, this session cannot
self-approve.

---

## Gate status

> **Phases 13-16 MUST NOT author any `crates/executor` or `crates/brokerd` file implementing
> CONTENT-01, SMTP-05, or CONFIRM-03 until this record shows Decision: APPROVED and Gate status:
> UNBLOCKED.**

**crates/executor / crates/brokerd (CONTENT-01 / SMTP-05 / CONFIRM-03 additions) is: BLOCKED**

Available resolutions: [ UNBLOCKED / BLOCKED ]

No executor/TCB code for CONTENT-01, SMTP-05, or CONFIRM-03 exists in the repo as of this record
(round-1 fixes applied, 2026-07-07; commits `5dc1e67`, `92c6487`) — consistent with this phase's
documentation-only scope. Gate status stays BLOCKED through the round-1 fix→re-review loop; it is set
to UNBLOCKED ONLY after caprun-opus-77's round-2 fresh-context review resolves with no unresolved
blocker/major and Decision is set to APPROVED. Round 1 finding a BLOCKER (finding #1, the taint-
laundering provenance defect) and being fixed is the expected, successful "gate earns its cost"
outcome — as v1.2's B1 was — not a process failure.
