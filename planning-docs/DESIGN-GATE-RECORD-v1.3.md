# DESIGN Gate Record — v1.3

**Date:** 2026-07-07
**Reviewer:** TBD — fresh-context reviewer arranged by caprun-opus-77 (D-11; MUST NOT be
caprun-sonnet-77, the session that authored `DESIGN-content-adapter-mediation.md` and
`DESIGN-confirm-binding.md`)
**Phase:** 12-content-adapter-confirm-binding-design-gate — Plan 03
**Review round:** 1 (initial adversarial review)

## Documents Under Review

| Document | sha256 |
|----------|--------|
| `planning-docs/DESIGN-content-adapter-mediation.md` | `c2506396852d4bd619d7985cf2973cdd3b140177cff3c5d82f53038b3fa6724c` |
| `planning-docs/DESIGN-confirm-binding.md` | `c7a614233324f8a3d012a27836e4b891f27f2aff4197bcbd8d85e3db65b3f1f2` |

Hashes were computed with `shasum -a 256` at gate-record authoring time. The reviewer MUST
re-run `shasum -a 256 planning-docs/DESIGN-content-adapter-mediation.md planning-docs/DESIGN-confirm-binding.md`
and confirm the values match before setting Decision: APPROVED. If the DESIGN docs are amended
during a fix→re-review loop, re-hash them here and note the round.

*(The exact hex digests are computed and inserted by the executor immediately below, at
authoring time, per the same convention as `DESIGN-GATE-RECORD-v1.2.md`'s table — see the
`<!-- shasum -->` block.)*

<!-- shasum -->
```
$ shasum -a 256 planning-docs/DESIGN-content-adapter-mediation.md planning-docs/DESIGN-confirm-binding.md
c2506396852d4bd619d7985cf2973cdd3b140177cff3c5d82f53038b3fa6724c  planning-docs/DESIGN-content-adapter-mediation.md
c7a614233324f8a3d012a27836e4b891f27f2aff4197bcbd8d85e3db65b3f1f2  planning-docs/DESIGN-confirm-binding.md
```
(Run at authoring time, 2026-07-07; values recorded in the "Documents Under Review" table above.)

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

- [x] **Item 3 — Precedence between routing-block and body-block is explicit** (CONTENT-01, D-02)
  - Grep matched: `grep -ci 'precedence'` → 6; `grep -c 'Precedence — Routing-Block vs Body-Block'` → 1.

- [x] **Item 4 — Collect-then-Block: per-arg loop collects all sensitive+tainted args before
  any Block; decision/anchor types become plural** (D-14)
  - Grep matched: `grep -c 'Collect-then-Block'` → 2; `grep -ci 'plural'` → 2;
    `grep -c 'Vec<BlockedArg>'` → 2.

- [x] **Item 5 — Worker-never-sends; SMTP secrets live only in the broker** (SMTP-01/02, D-03/D-04)
  - Grep matched: `grep -c 'Worker-never-sends'` → 1; `grep -c 'Secrets broker-only'` → 1;
    `grep -c 'crates/brokerd/src/sinks/email_smtp.rs'` → 2.

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

- [x] **Item 9 — Plan-node API confirmed untouched; no `EffectRequest` path introduced** (D-18)
  - Grep matched: `grep -c 'Plan-Node API Is Untouched'` → 1; `grep -c 'EffectRequest'` → 1
    (mentioned only to state it is NOT introduced — annotate per `check-invariants.sh` Gate 1
    if this trips the repo-wide token gate).

### DESIGN-confirm-binding.md

- [x] **Item 10 — Combined SHA-256 digest over the FULL SET of blocked args' literals, never
  per-arg or subset digests** (CONFIRM-03, D-08/D-19)
  - Grep matched: `grep -c 'Combined-Digest Binding'` → 1; `grep -ci 'combined_digest'` → 8;
    `grep -ci 'never per-arg\|never a per-arg digest'` → 1.

- [x] **Item 11 — Post-transformation-bytes rule: mint after transform, no drift between
  confirm and send** (CONFIRM-03, D-08, Pitfall 2)
  - Grep matched: `grep -c 'Post-Transformation Bytes'` → 1; `grep -ci 'mint-after-transform\|mint only AFTER'` → 2;
    `grep -c 'Why this closes D-12(b)'` → 1.

- [x] **Item 12 — No-truncation verbatim display for every blocked arg** (CONFIRM-03, D-09)
  - Grep matched: `grep -c 'Verbatim Display — No Truncation'` → 1; `grep -ci 'no truncation\|MUST NOT truncate'` → 2.

- [x] **Item 13 — Block narration covers every blocked arg individually, in canonical order
  matching the digest** (CONFIRM-04, D-20)
  - Grep matched: `grep -c 'Block Narration for Every Arg'` → 1; `grep -ci 'never only the first-matched arg'` → 1.

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
  - **Status:** OPEN — pending fresh-context adversarial pass. NOT resolved by this authoring
    session (D-11).

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
    `crates/brokerd/src/confirmation.rs:100-124` extension) is genuinely frozen at Block time and
    never re-derived at confirm time — the doc's own MUST NOT statement to that effect must be
    checked against the schema, not merely quoted.
  - **Status:** OPEN — pending fresh-context adversarial pass. NOT resolved by this authoring
    session (D-11).

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
  - **Status:** OPEN — pending fresh-context adversarial pass. NOT resolved by this authoring
    session (D-11).

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

**Decision:** NEEDS REVISION (pending fresh-context adversarial review — see Task 2)

This scaffold (Task 1) intentionally initializes Decision to NEEDS REVISION, not APPROVED. Per
D-11, the session that authored `DESIGN-content-adapter-mediation.md` and
`DESIGN-confirm-binding.md` (this session, caprun-sonnet-77) MUST NOT also review or approve them.
Task 2 is a blocking-human checkpoint: it stops here, flags caprun-opus-77 via FAMP to arrange a
fresh-context reviewer, and only that reviewer's sign-off (recorded per the How-to-Verify steps
above) may change this field to APPROVED.

---

## Gate status

> **Phases 13-16 MUST NOT author any `crates/executor` or `crates/brokerd` file implementing
> CONTENT-01, SMTP-05, or CONFIRM-03 until this record shows Decision: APPROVED and Gate status:
> UNBLOCKED.**

**crates/executor / crates/brokerd (CONTENT-01 / SMTP-05 / CONFIRM-03 additions) is: BLOCKED**

Available resolutions: [ UNBLOCKED / BLOCKED ]

No executor/TCB code for CONTENT-01, SMTP-05, or CONFIRM-03 exists in the repo as of this record's
authoring (Task 1, 2026-07-07) — consistent with this phase's documentation-only scope. This field
is set to UNBLOCKED ONLY by Task 2, after the fresh-context adversarial review resolves any
blocker/major and Decision is set to APPROVED.
