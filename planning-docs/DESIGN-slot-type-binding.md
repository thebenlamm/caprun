# DESIGN — Slot-Type Binding Enforcement (T2)

**Milestone:** v1.5 — Slot-Type Binding Enforcement
**Phase:** 23 (Design Gate) — blocks all `crates/executor` / `crates/brokerd` mint-site code for this milestone
**Status:** Draft → pending fresh (non-self) adversarial review (see `DESIGN-GATE-RECORD-v1.5.md`)
**Author date:** 2026-07-11
**Grounding:** `.planning/phases/23-slot-type-binding-design-gate/23-RESEARCH.md` (every file:line below traces to a direct code read; re-verify if Phase 24 begins many commits later)
**Requirements:** DESIGN-07, DESIGN-08, DESIGN-09, DESIGN-10 (this doc) → enables T2-02..05 (Phase 24), T2-06..08 (Phase 25)

> **Design-gate discipline.** No `crates/executor` or `crates/brokerd` mint-site/TCB code
> for slot-type binding may be written until this document clears a fresh, non-self
> adversarial review with every finding resolved — mirroring v1.0 Phase 2, v1.2 Phase 8,
> v1.3 Phase 12, and v1.4 Phase 18. This doc pins **decisions**, not options; Phase 24 is a
> mechanical realization of what is fixed here.

---

## §0. Purpose & Scope

**The gap (v1.4 residual T2).** The executor's I2 check blocks an *attacker-tainted* value in a
sensitive sink-arg slot, and I0/I1 demote a session that touched untrusted content. Neither
fires when a **`UserTrusted`** value is minted for one semantic field and then routed into a
**different** slot — e.g. a value minted as an email `body` (trusted, non-tainted) submitted
into the `to` slot. It is neither untrusted (I2 silent) nor a class-level deny (I0/I1 silent).
v1.4's adversarial-planner proof left this as the last unenforced degree of freedom and
disclosed it as residual #5.

**What this milestone adds.** A structural check that a resolved value's **semantic origin role**
matches the **expected role** of the plan-node slot it is routed into. A misrouted
`UserTrusted` handle is caught by a new deterministic `Denied`.

**Mechanism, in one sentence.** Every minted value carries an additive `origin_role` tag; the
executor holds a hardcoded per-sink-arg expected-role table; `submit_plan_node` denies a plan
node when a resolved value's role does not match its slot's expected role — fail-closed.

**In scope (v1.5):** the two live sinks `email.send` and `file.create`; an additive tag through
the three existing mint sites; a new exhaustive `DenyReason` variant; the enforcement point in
`submit_plan_node`.

**Explicitly out of scope (locked at scoping, `.planning/REQUIREMENTS.md` Out-of-Scope):**
- Any change to I0/I1 **trust classification** — which values become `UserTrusted` vs untrusted
  is unaffected; this adds an orthogonal tag only.
- A general/pluggable role framework, config-file policy, or rule engine — the table stays
  hardcoded in the Rust TCB, mirroring `sink_sensitivity.rs` (CON-i2-non-bypassable).
- Sinks beyond `email.send` / `file.create`.
- Connection/capability-model changes (shipped in v1.4 Phase 20).

---

## §1. Origin-Role Tag Mechanism (DESIGN-07a)

**Decision.** Add one additive field to the broker-owned value record:

```rust
// crates/runtime-core/src/value_record.rs:21-31 — ValueRecord today:
//   { id: ValueId, literal: String, taint: Vec<TaintLabel>, provenance_chain: Vec<Uuid> }
pub origin_role: Option<String>,   // ADD — additive, no existing shape changes
```

- **`Option<String>`, not bare `String`.** `None` must be representable as a state distinct from
  every valid role tag, because DESIGN-10's fail-closed default keys off exactly that bit
  (`None` never matches a slot's expected-role list). Bare-`String` style matches the existing
  `claim_type: String` convention rather than inventing a new enum.
- **Lives on `ValueRecord`, not a side-table and not a `TaintLabel` variant.**
  - Not a side-table keyed by `ValueId`: `resolve()` already returns everything the executor
    needs in one call (`crates/executor/src/value_store.rs:89-91`); a second lookup adds a
    can-go-stale surface for no benefit — the field is small and always co-resident with
    taint/provenance.
  - Not a new `TaintLabel` variant: `TaintLabel` is the I0/I1 trust vocabulary — the eight
    variants `ExternalUntrusted`, `EmailRaw`, `PathRaw`, `PdfRaw`, `LlmGenerated`,
    `WorkerExtracted`, `UserTrusted`, `LocalWorkspace` (`plan_node.rs:13-24`; corrected after
    DESIGN-GATE-RECORD-v1.5 Round 1 finding F5 — the argument is unaffected by the exact count,
    and `is_untrusted()`'s no-wildcard match at `plan_node.rs:40` would be polluted by a role
    variant). Folding role into it would conflate "is this value trustworthy" (I0/I1)
    with "is this value the right shape for this slot" (T2) — and Phase 24's mandate is explicit
    that I0/I1 classification is UNAFFECTED. A parallel field keeps the two concerns orthogonal
    by construction.
- **Assigned atomically at mint time**, by the broker (sole mint authority, T-04-03), the same
  way taint/provenance are assigned today — never by the planner, never post-hoc.
- **Serde compatibility (added after DESIGN-GATE-RECORD-v1.5 Round 1, finding F6, NIT):**
  `ValueRecord` derives `Deserialize`; Phase 24 should annotate the new field
  `#[serde(default)] pub origin_role: Option<String>` so any record serialized before the field
  existed still deserializes (`None`). Low stakes — `ValueRecord` is not observed crossing the IPC
  wire — but free insurance.

The tag is populated at the three mint sites (§4 and §9). This is an additive signature change
at existing call sites; it does **not** add a new `mint_from_read(` / `mint_from_derivation(` /
`.mint(` call-site token, so it does not violate `check-invariants.sh` Gate 3 (§9, Landmines).

---

## §2. Role Vocabulary & `claim_type` Unification (DESIGN-08)

**Decision.** ONE string-tag vocabulary, **reused** across both origins — not a parallel taxonomy.

**Untrusted-origin values (`mint_from_read`).** `origin_role` is the SAME string already carried
in `claim.claim_type`, reused verbatim. Today the `claim_type` taxonomy in
`crates/brokerd/src/quarantine.rs` is consumed at mint time to derive taint labels and then
**discarded** — it never reaches `ValueRecord`. That discard IS the gap DESIGN-08 closes: the
semantic type already exists; we let it survive on the record instead of inventing a second name
for it.

| `claim_type` value | Set at | Reused as `origin_role` |
|---|---|---|
| `"email_address"` | `extract_email_claims` (`quarantine.rs:79`) | `"email_address"` |
| `"relative_path"` | `extract_relative_path_claims` (`quarantine.rs:110`) | `"relative_path"` |
| `"doc_fragment"` | `extract_doc_fragments` (`quarantine.rs:176`) | `"doc_fragment"` |

Any other `claim_type` is already a fail-closed mint error (`quarantine.rs:336-340`) — so no
untrusted value can carry an unknown role.

**`UserTrusted`-origin values (`mint_from_intent`), defined from scratch per DESIGN-08.** These
have no `claim_type` today; the role is keyed by which `server.rs` call site mints the literal
(each site already knows its own semantic field, `server.rs:1294-1300`):

| Intent field | Mint call site | `origin_role` |
|---|---|---|
| recipient | `server.rs:1317` (`SendEmailSummary` arm) | `"recipient"` |
| subject | `server.rs:1330` | `"subject"` |
| body | `server.rs:1347` | `"body"` |
| path (`CreateFileFromReport`) | `server.rs:1317` (`CreateFileFromReport` arm) | `"path"` |

> **Revised after DESIGN-GATE-RECORD-v1.5 Round 1, finding F3 (MINOR).** The recipient and the
> path are minted through the SAME `mint_from_intent` call site (`server.rs:1317` mints
> `primary_literal`), which is the recipient for `SendEmailSummary` but the path for
> `CreateFileFromReport` — the two are distinguished by the intent-variant `match` at
> `server.rs:1294-1300`, not by two separate call sites. **Phase 24 MUST select the role inside
> that intent-variant match and thread it to the `:1317` call — never hardcode `"recipient"` at
> the call site.** Hardcoding `"recipient"` at `:1317` would tag a legitimate `file.create` path
> `"recipient"`, and §3's `path => ["path","relative_path"]` slot would then fail-closed-Deny a
> valid path flow (a fail-closed regression, not a security hole, but a real correctness bug).

**Derivation output (`mint_from_derivation`):** `"recipient"` — see §4.

**Deliberate dual-vocabulary tradeoff.** The untrusted spellings (`"email_address"`) and the
trusted spellings (`"recipient"`) both mean "belongs in a `to`/`cc`/`bcc` slot" but are spelled
differently. Rather than add a translation layer (a canonical vocabulary + a `claim_type`→role
map, with its own correctness burden and zero behavioral gain), the expected-role table (§3)
**enumerates both spellings** per slot as a flat, greppable array. Explicit and auditable beats
a hidden mapping function.

---

## §3. Expected-Role Table (mirrors `sink_sensitivity.rs`)

**Decision.** A hardcoded `&str`-keyed lookup in `crates/executor`, structurally identical to the
existing `is_routing_sensitive` / `is_content_sensitive` precedent
(`crates/executor/src/sink_sensitivity.rs:86-107`, doc-commented "a security property, not a
configuration knob. CON-i2-non-bypassable"). No config file, no framework.

**Contract (the exact return type is load-bearing — see §7 and §8 angle 2):**

```rust
// New fn, same file/crate as sink_sensitivity.rs, same hardcoded-match discipline.
fn expected_role(sink: &SinkId, arg_name: &str) -> Option<&'static [&'static str]>
//   None            => this slot is UNCONSTRAINED for v1.5 — role check is a no-op (NOT fail-open; see §7 item 3)
//   Some(&[roles])  => this slot IS role-checked — value's origin_role MUST be one of `roles`
//   Some(&[])       => MUST NEVER be constructed (a slot with zero valid roles is a design bug, not a runtime state)
```

Slot keys are `arg.name` (`PlanArg { name: String, .. }`, `plan_node.rs:108-115`) — the same
`&str` keys `sink_sensitivity.rs` already uses (`"to"`, `"cc"`, `"bcc"`, `"subject"`, `"body"`,
`"path"`, `"contents"`). No new identity concept.

**Table contents (pinned — not "TBD"; §8 angle 5 requires exact membership):**

| Sink | Arg | Expected roles | Rationale |
|---|---|---|---|
| `email.send` | `to`, `cc`, `bcc` | `["recipient", "email_address"]` | trusted recipient OR a legitimately doc-derived one (§4); both spellings enumerated per §2 |
| `email.send` | `subject` | `["subject"]` | |
| `email.send` | `body` | `["body", "doc_fragment"]` | trusted `body` OR a doc-derived summary; **corrected in Phase 24** — see amendment note below |
| `file.create` | `path` | `["path", "relative_path"]` | trusted path OR an extracted relative path |
| `file.create` | `contents` | `None` (unconstrained, v1.5) | no known-safe role vocabulary for arbitrary file content; Assumption A2 |
| any | any other arg | `None` (unconstrained) | |
| any other sink | any | `None` (unconstrained) | out of v1.5 scope |

The `["recipient", "email_address"]` membership is what lets BOTH a human-typed recipient and a
legitimately concat-derived one pass the `to` slot, while still catching a MISROUTED `body`- or
`subject`-tagged value there.

> **Revised after DESIGN-GATE-RECORD-v1.5 Round 1, finding F4 (MINOR — load-bearing invariant).**
> Untrusted-origin role tags are **worker-influenced**: the worker picks the `WorkerClaim` variant
> (`proto.rs:24-43`), and `mint_from_read` shape-validates only `doc_fragment`
> (`quarantine.rs:327`) — so a hostile worker can mint an arbitrary string tagged
> `"email_address"` or `"relative_path"`. The role tag is therefore NOT a trust signal on the
> untrusted side. This design is safe only because of a **table-construction invariant** that must
> hold and is hereby pinned:
>
> **An untrusted-origin role tag (`"email_address"`, `"relative_path"`, `"doc_fragment"`) may
> appear in a slot's expected-role list ONLY if that slot is also routing- or content-sensitive**
> (`is_routing_sensitive || is_content_sensitive`, `sink_sensitivity.rs`). Because such a value
> carries untrusted taint, I2's per-arg Block (Step 2/3) fires there regardless of role — so the
> role check never becomes the *sole* gate for an attacker-controllable value. The role check's
> job is to catch a misrouted **`UserTrusted`** value (the T2 gap); untrusted values remain
> governed by I2. Every untrusted role in §3's table (`email_address`@`to/cc/bcc`,
> `relative_path`@`path`, `doc_fragment`@`body`) satisfies this — `to/cc/bcc` and `path` are
> routing-sensitive, and `body` is content-sensitive.

> **Amended during Phase 24 execution (2026-07-11) — `body` row corrected `["body"]` → `["body", "doc_fragment"]`.**
> The Round-1 pin `["body"]` referenced a `"body"` claim_type that **does not exist in the live
> code**. Body content reaches `email.send` as `WorkerClaim::DocFragment` →
> `claim_type: "doc_fragment"` → `origin_role: "doc_fragment"` (traced through `worker.rs`
> `SendEmailSummary` → `server.rs` → `mint_from_read`). The original `["body"]` would have
> hard-Denied **every** legitimate body flow; three pre-existing tests in
> `crates/brokerd/tests/extract_provenance_threading.rs` empirically confirmed this once Step 1c
> landed. The correction is safe under the load-bearing invariant above: `body` is
> content-sensitive, so a `doc_fragment`-tagged (always-untrusted) value there still fires I2's
> per-arg Block — the role check is not the sole gate. The exfiltration-critical recipient slots
> (`to`/`cc`/`bcc`) were **not** touched and still reject `doc_fragment`. Independently
> re-confirmed by a fresh adversarial code-trace review (no blocker) and the Phase 24
> gsd-verifier (4/4 must-haves). Pending final independent confirmation in Phase 25's live-Linux
> proof. See `.planning/phases/24-slot-type-binding-enforcement/24-03-SUMMARY.md` for the full
> evidence trail.

---

## §4. Derivation Role Propagation (DESIGN-09)

**Decision.** `mint_from_derivation`'s `origin_role` is a deterministic function of
`transform_kind` — NOT inherited or unioned from the input values' roles. Computed in the SAME
`match transform_kind { "concat" => ..., other => fail-closed }` block that already exists
(`quarantine.rs:670-692`). For `"concat"`, hardcode `origin_role = Some("recipient")`.

**Grounding.** `TransformKind` (`crates/brokerd/src/proto.rs:57-61`) has exactly ONE variant
today: `Concat` — a fixed `'@'`-join over doc fragments, mapped via `as_mint_tag()` to the tag
`"concat"`. Its byte-verify guard proves the output literal is exactly `join(inputs, '@')`
(`quarantine.rs:671-685`).

> **Revised after DESIGN-GATE-RECORD-v1.5 Round 1, finding F2 (MINOR).** Precisely: `Concat`
> joins N inputs on `'@'` and rejects only the zero-input case (`quarantine.rs:588-593`) — so 1
> input yields the input verbatim (no `@`), 2 inputs yield `local@domain`, 3+ yield `a@b@c`. The
> `local@domain` email shape is guaranteed only for the **2-input** case (the live Reply-To/Domain
> flow). Phase 24 MUST therefore either (a) enforce `inputs.len() == 2` for the `"concat"` arm
> before assigning `"recipient"`, or (b) rely on the residual cover that the derived value is
> unconditionally `WorkerExtracted`/untrusted (`quarantine.rs:611-613`), so I2's per-arg Block
> fires at any routing/content-sensitive slot regardless of role — meaning a degenerate-arity
> concat can never reach a sensitive sink as `Allowed` even if mis-shaped. (a) is preferred as the
> tighter guarantee. The `"recipient"` role assignment below is grounded in the intended 2-input
> flow with I2 as the backstop for anything else.

In the live flow this is the Reply-To/Domain doc-fragment pair being
assembled into a recipient candidate — literally the v1.4 T2 scenario this milestone closes. The
derived value's taint is unconditionally untrusted (`WorkerExtracted` forced in, `UserTrusted`
dropped — `quarantine.rs:595-624`), so I2's per-arg Block already fires whenever it lands in a
routing/content-sensitive slot, independent of role.

**Why `"recipient"` and not `None`.** Tagging the concat output `None` would be *wrong*, not
merely cautious: it would make every legitimately-derived recipient unconditionally
fail-closed-Deny at the `to` slot (§7), breaking the exact concat-derived-recipient flow this
milestone's own acceptance path exercises (Assumption A1 — cross-checked during authoring against
the confirm-binding framing of this scenario).

**Why role is NOT inherited from inputs (anti-laundering, §8 angle 1).** The two doc-fragment
inputs each carry `origin_role = "doc_fragment"`. The recommendation assigns a NEW role from the
transform's own **verified output shape**, and never reads `inputs[*].origin_role`. This closes a
laundering path where an attacker could smuggle a `"recipient"`-tagged input in to make the
output inherit an unearned role. Note the contrast with taint, which IS unioned across inputs
(`quarantine.rs:603-613`) — a naive implementer will reach for the same union shape for role;
the doc forbids it (§10 Pitfall 3).

**Forward-looking.** A future SECOND `TransformKind` variant forces a compile-time decision at
this same no-wildcard match — consistent with the codebase's exhaustive-match discipline. This is
a named v2 obligation, not something to design for now.

---

## §5. New `DenyReason` Variant & Exhaustive-Match Blast Radius (DESIGN-07b)

**Decision.** Add ONE variant to the exhaustive `DenyReason` taxonomy
(`crates/runtime-core/src/executor_decision.rs:15-60`), no wildcard arm (per the project's §10
exhaustive-match discipline):

```rust
SlotTypeMismatch { sink: String, arg: String, expected: Vec<String>, found: Option<String> },
```

> **Revised after DESIGN-GATE-RECORD-v1.5 Round 1, finding F1 (MAJOR).** The field types MUST be
> owned (`Vec<String>`, not `&'static [&'static str]`). `DenyReason` derives
> `serde::Deserialize` (`executor_decision.rs:14`) and the decision crosses the IPC wire (the
> worker deserializes the decision, `worker.rs:370-381`) — serde cannot deserialize into
> `&'static` references. The `expected` list is *populated from* the static expected-role table
> (§3) into an owned `Vec<String>` at construction time; the table itself stays `&'static`. This
> is a doc-level correction to the pinned shape, not a design change.

Name `SlotTypeMismatch` to match `ROADMAP.md` / `REQUIREMENTS.md` vocabulary verbatim (reduces
doc-to-requirement traceability friction). The recommendation reuses the EXISTING
`ExecutorDecision::Denied { reason }` carrier — it does NOT add a new `ExecutorDecision` variant,
so the outer enum's match sites are untouched. (Confirmed at review: the only non-test match over
`ExecutorDecision` is `server.rs:650-683`, which carries a `_ =>` wildcard arm, so a reused
`Denied { reason }` needs no update there — Assumption A3 discharged.)

**Full exhaustive-match blast radius.** A workspace-wide `grep -rn "DenyReason" crates/ cli/`
(all non-`/target/` hits reviewed; independently re-confirmed during this authoring) found
**exactly TWO exhaustive matches over the enum**, both co-located with its definition:

| # | File:line | Match | Update for the new variant |
|---|---|---|---|
| 1 | `crates/runtime-core/src/executor_decision.rs:64-80` | `impl DenyReason { pub fn code(&self) -> &'static str }` | add `"slot_type_mismatch"` arm |
| 2 | `crates/runtime-core/src/executor_decision.rs:83-112` | `impl std::fmt::Display for DenyReason` | add a human-readable arm |

**Every other `DenyReason` reference is a *construction* site, not a match** — these need NO
update; Phase 24 only ADDS one new construction site (§6, Step 1c in `submit_plan_node`):
`sink_schema.rs:113-136`, `executor/src/lib.rs:85/98/107/180/205`, `brokerd/src/lib.rs:80-91`
(test), and several test files. **`cli/caprun/src/worker.rs:381` uses
`matches!(decision, ExecutorDecision::Allowed)` + `eprintln!("{decision:?}")` (Debug, auto-derived)
— NOT an exhaustive `DenyReason` match; it needs NO update.** Audit persistence uses
`#[derive(Serialize)]`, not a hand-written match — also no update.

> **Phase 24 MUST re-confirm this independently.** This blast-radius count (exactly 2 sites) is a
> claim about the code at authoring time, not a licence to skip verification. Phase 24's first
> task is to re-run `grep -rn "DenyReason" crates/ cli/` and confirm the count before adding the
> variant. `check-invariants.sh` (no-wildcard discipline) is the compile-time backstop: if a
> third exhaustive match exists and is missed, the build fails to compile rather than silently
> mis-rendering. Treat 2 as the expected, not the guaranteed, answer.

**Do not conflate with `SessionStatus`.** The exhaustive `SessionStatus` match at
`executor/src/lib.rs:176` (also no-wildcard) is a SEPARATE match and is UNCHANGED by this phase —
Step 1c does not touch session status (§8 angle 6, Landmines).

---

## §6. Ordering Ruling — Step 1c, hard `Denied` (DESIGN-07c)

**Decision.** The role check is a NEW per-arg fail-closed structural guard ("Step 1c"),
returning a hard `Denied { reason: SlotTypeMismatch { .. } }` on first mismatch — it does **NOT**
join the collect-then-Block `BlockedPendingConfirmation` set.

**Why hard `Denied`, never confirmable.** A role mismatch is a STRUCTURAL/type error, not a
judgment call a human can soundly resolve. Contrast a tainted-but-plausible recipient, where a
human confirming "yes, send to this externally-sourced address" is a meaningful action —
`BlockedPendingConfirmation` exists precisely for that confirmable case (D-14). There is no sound
human response to "this value's role doesn't match its slot" other than fixing the plan node and
resubmitting. Reusing the confirm path for a non-confirmable violation would be a category error
and would force a new `BlockedArg` / `SinkBlockedAnchor` shape (or a role field bolted onto the
existing one). Step 1c requires **zero** new anchor shapes — only the §5 `DenyReason` variant.

**Placement — the full existing `submit_plan_node` step order** (`crates/executor/src/lib.rs:54-216`),
with Step 1c inserted so a reviewer can diff old-vs-new line by line:

| Step | Location | Behavior | Change |
|---|---|---|---|
| 0 — schema gate | `:66-68` | `validate_schema`; hard `Denied`, immediate return | unchanged |
| **per-arg loop** | `:78-158` | for each `PlanArg`: | |
| 1 — resolve handle | `:81-88` | `None` → hard `Denied { DanglingHandle }`, return | unchanged |
| 1a — empty-taint | `:96-100` | hard `Denied`, return | unchanged |
| 1b — empty-provenance | `:105-109` | hard `Denied`, return | unchanged |
| **1c — role check (NEW)** | between 1b and 2/3 | mismatch → hard `Denied { SlotTypeMismatch }`, **return** | **added** |
| 2/3 — sensitivity | `:117-157` | `is_routing_sensitive \|\| is_content_sensitive` AND untrusted → **collect** into `blocked` | unchanged |
| after loop | `:162-164` | `blocked` non-empty → `BlockedPendingConfirmation`, return | unchanged |
| 0.5 — I0 class-deny | `:174-213` | exhaustive `SessionStatus` match; Draft/non-live + `CommitIrreversible` → hard `Denied` | unchanged |
| Allowed | `:215` | only if nothing above returned | unchanged |

**Precedence preserved (Phase 24 success criterion 4).** Step 1c sits in the same tier as the
existing structural guards (1/1a/1b) and fires per-arg BEFORE that arg is considered for
sensitivity collection. So:
- I2 Block still fires exactly as before for every arg that PASSES the role check.
- I2-before-I0 is untouched: Step 0.5 (I0) still runs only after the per-arg loop completes with
  an empty `blocked` set — the load-bearing comment at `:169-172` ("the per-arg I2 Block always
  takes precedence over this I1/I0 class-level deny") remains true, because Step 1c is a per-arg
  guard, not a class-level one.

**New total order:** schema (0) > structural per-arg guards incl. NEW role check (1/1a/1b/1c) >
I2 sensitivity Block (2/3, collect-then-Block) > I0 class-deny (0.5) > Allowed.

---

## §7. Fail-Closed Default (DESIGN-10)

**Decision.** At a role-checked slot, a value with **no** role or a **non-matching** role is a
`Denied` — never a silent pass-through to `Allowed`. Concretely, given
`expected_role(sink, arg)` returning `Option<&[&str]>` and `record.origin_role: Option<String>`:

1. Slot role-checked (`Some(list)`) AND value has no role (`origin_role == None`) → **`Denied`**.
2. Slot role-checked (`Some(list)`) AND value's role ∉ `list` → **`Denied`**.
3. Slot unconstrained (`expected_role` returns `None`, e.g. `file.create`'s `contents`) → role
   check is a **no-op** for that arg; fall through to Step 2/3 as today. **This is NOT fail-open**
   — it is a deliberately scoped-out slot (§3, Assumption A2). The doc states this explicitly at
   the DOCUMENT level (not only in code) to satisfy DESIGN-10's "never silent pass-through"
   language: an unconstrained slot is a *documented, intentional* absence of a check, not an
   accidental gap.

**The `Option` vs empty-slice contract is load-bearing (§8 angle 2, §10 Pitfall 2).** The lookup
MUST distinguish "no check for this slot" (`None`) from "this slot allows a specific set"
(`Some(list)`). `Some(&[])` (a slot that allows nothing) must never be constructed — a zero-valid-role
slot is a design bug, not a runtime state. In particular, Phase 24 MUST NOT implement the lookup
as `.unwrap_or(&[])`, which would collapse the two states and turn an unconstrained slot into a
"deny everything" slot (or, depending on the comparison, an "allow anything" slot) — either way
breaking the fail-closed contract. A `.unwrap_or(&[])` anywhere in the Phase 24 lookup is a
review red flag.

---

## §8. Adversarial-Review Preemption

A fresh non-self reviewer (Phase 23's gate, DESIGN-07) will probe these; each is addressed above
and is answerable by tracing the cited code, not by trusting this doc:

1. **Can a derived value launder its role?** No — §4: role is assigned from `Concat`'s
   byte-verified output shape (`local@domain`), never inherited/unioned from input roles. The
   inputs' `"doc_fragment"` roles are never read by the role assignment.
2. **Does fail-closed truly fail closed for an unassigned role?** §7: BOTH failure shapes
   (`None` role at a role-checked slot; role ∉ list) reach `Denied`, with no third path. The
   `Option<&[&str]>` contract (not `.unwrap_or(&[])`) is pinned to close the empty-slice-means-allow-anything
   ambiguity.
3. **Does the check reorder/weaken I0/I2?** §6: the full existing step list is reproduced with
   Step 1c inserted between 1b and 2/3; Step 0.5 (I0) is untouched and still gated on an empty
   `blocked` set from Steps 2/3 (I2). Reviewer can diff old-vs-new ordering line by line.
4. **Is the match blast-radius actually complete?** §5: the full grep result is given (exactly 2
   sites), with the explicit note that `worker.rs` uses `matches!`/Debug (no update) and audit
   uses `#[derive(Serialize)]` (no update). Reviewer can independently re-run
   `grep -rn "DenyReason" crates/ cli/`. Phase 24 is required to re-confirm; the no-wildcard
   `check-invariants.sh` discipline is the compile-time backstop.
5. **Does the `claim_type`-vs-role dual vocabulary create a bypass?** §2/§3: the exact
   `Some(&[...])` membership per slot is pinned (not "TBD"), so a reviewer can check for
   over-broad membership. An attacker-controlled `doc_fragment` value carries role
   `"doc_fragment"`, which is NOT in any `email.send` routing slot's list — so it cannot be
   routed into `to` even if its bytes look address-shaped (and if it were concat-derived, it is
   untrusted and I2 already blocks it).
6. **Session-status interaction:** §6: Step 1c lives inside the per-arg loop that runs
   unconditionally BEFORE Step 0.5's session-status match — a Draft session's role mismatch
   still Denies at Step 1c. `SessionStatus`'s own exhaustive match is separate and unchanged.

---

## §9. Phase 24 / 25 Implementation Map (informative — not part of the gate)

Anticipates the mechanical work the gate unblocks. Grounded, but Phase 24 re-verifies file:line.

**Mint sites — additive `origin_role` threading (T2-02):**

| Mint site | Signature today | Role source |
|---|---|---|
| `mint_from_read` (`quarantine.rs:284-297`) | `(conn, store, session_id, claim, parent_id, parent_hash)` | `claim.claim_type` verbatim (§2) — already resolved inside the fn |
| `mint_from_intent` (`quarantine.rs:435-442`) | `(conn, store, session_id, literal, parent_id, parent_hash)` | caller-supplied at the 3 `server.rs` sites (recipient/subject/body) + the `path` arm (§2) |
| `mint_from_derivation` (`quarantine.rs:574-583`) | `(conn, store, session_id, transformed_literal, inputs, transform_kind, parent_id, parent_hash)` | from `transform_kind` (`"concat"`→`"recipient"`, §4) |

**Mint-ordering landmine.** `mint_from_read` appends its `file_read` Event BEFORE minting
(`quarantine.rs:342-366`); `mint_from_derivation` mints the value FIRST, then appends its
`derivation` Event (`quarantine.rs:702-729`, whose hashed payload embeds the just-minted
`value_id`). Phase 24 code threading `origin_role` through both MUST NOT assume a uniform
mint-then-event order.

**Executor (T2-03/04/05):** new `expected_role()` in `sink_sensitivity.rs` (§3); new
`SlotTypeMismatch` `DenyReason` variant + the 2 exhaustive-match updates (§5); new Step 1c
construction site in `submit_plan_node` (§6).

**Gate invariants Phase 24 must not trip:** `check-invariants.sh` Gate 3 restricts
`mint_from_read(` / `mint_from_derivation(` / `.mint(` call-site tokens to `quarantine.rs`,
`server.rs`, `value_store.rs` — a signature change is safe; a NEW call site elsewhere fails the
build. Gate 1 forbids `EffectRequest` under `crates/` — keep the table a
`(SinkId, arg_name) -> Option<&[&str]>` lookup, never a raw args-map-to-sink path.

**Phase 25 (T2-06/07/08):** the held-out swapped-recipient↔subject acceptance test must live
behind `#[cfg(target_os = "linux")]` and run via `scripts/mailpit-verify.sh`, not bare
`cargo test` on the Mac dev machine; the executor-level `Denied` path itself is workspace-buildable
and not Linux-gated (`cargo test -p executor executor_decision`). A regression audit updates
existing fixtures that rely on permissive `UserTrusted`-in-any-slot behavior
(`executor_decision.rs`, `brokerd` mint-threading fixtures).

---

## §10. Accepted Residual Risks, Assumptions & Common Pitfalls

**Accepted residual risks (v1.5):**
- `file.create`'s `contents` arg is role-**unconstrained** (§3, §7 item 3, Assumption A2). No
  known-safe role vocabulary exists for arbitrary file content this milestone. Documented, not
  accidental. A future milestone may add a `"file_body"` role mirroring `body`.
- The expected-role table is scoped to the two live sinks. Any third sink is unconstrained until
  its own design pass — consistent with `sink_sensitivity.rs`'s documented v0 sink scope.

**Assumptions (carried from research, to confirm during Phase 24, not silently):**
- **A1** — `"recipient"` is the correct hardcoded role for `Concat`-derived values (vs a generic
  `"derived_address"`). If wrong, legitimate concat-derived recipients would be denied at `to`.
  Cross-checked against the confirm-binding framing of this scenario.
- **A2** — `file.create`'s `contents` stays unconstrained for v1.5 (see residual risks).
- **A3** — no exhaustive match over the OUTER `ExecutorDecision` enum needs a new arm, because the
  recommendation reuses `Denied { reason }` rather than adding an `ExecutorDecision` variant. Low
  risk; Phase 24 should do a final `grep -rn "ExecutorDecision::" ` sanity pass anyway.

**Common pitfalls (for Phase 24 implementers):**
- **Treating role-mismatch as confirmable** — folding Step 1c into the `BlockedPendingConfirmation`
  collect loop. Warning sign: a `BlockedArg`/`SinkBlockedAnchor` gaining a `role` field instead of
  a dedicated `DenyReason` variant. §6 forbids it.
- **Empty-list-vs-`None` ambiguity** in `expected_role()`. Warning sign: `.unwrap_or(&[])`. §7
  forbids it.
- **Inheriting/unioning role across `mint_from_derivation` inputs** (mirroring how taint IS
  unioned). Warning sign: `origin_role` read from `inputs` inside `mint_from_derivation`. §4
  forbids it.

---

## Acceptance Predicate — Done When

Phase 23's gate is cleared when ALL are true:

1. This doc specifies the origin-role tagging mechanism (§1), the new `DenyReason` variant's shape
   AND its full exhaustive-match blast radius (§5), and the ordering ruling — hard `Denied` via
   Step 1c, not `BlockedPendingConfirmation` (§6). **(DESIGN-07)**
2. This doc unifies with the existing `claim_type` taxonomy for untrusted-origin values and
   defines from-scratch role tags for `ProvideIntent`-minted `UserTrusted` recipient/subject/body
   (§2). **(DESIGN-08)**
3. This doc explicitly resolves `mint_from_derivation` (`Concat`) role propagation — role from
   transform, hardcoded `"recipient"`, never inherited (§4). **(DESIGN-09)**
4. This doc pins the fail-closed default: no-role or unrecognized-role at a role-checked slot is a
   `Deny`, never a silent pass-through to `Allowed` (§7). **(DESIGN-10)**
5. This doc has cleared a fresh, non-self adversarial review (traced against real code) with every
   finding resolved (`DESIGN-GATE-RECORD-v1.5.md`), and no `crates/executor` / `crates/brokerd`
   mint-site code exists yet.

---

## Amendments (post-review)

Round-tagged amendments from the fresh adversarial review are folded into the relevant §above,
per `DESIGN-session-trust-coherence.md`'s convention. See `DESIGN-GATE-RECORD-v1.5.md` for the full
review.

**Round 1 (DESIGN-GATE-RECORD-v1.5, fresh independent Fable-5 reviewer, 2026-07-11):**
- **F1 (MAJOR) → §5:** `SlotTypeMismatch` field types changed from `&'static [&'static str]` to
  owned `Vec<String>` — `DenyReason` derives `Deserialize` and crosses the IPC wire; `&'static`
  refs are not deserializable. Also folded the A3 discharge (`server.rs:650-683` has a `_ =>` arm).
- **F2 (MINOR) → §4:** corrected the "`Concat` only produces `local@domain`" overstatement —
  `Concat` joins N inputs; the email shape holds for the 2-input case; Phase 24 enforces
  `inputs.len()==2` or relies on I2 as the residual cover for degenerate arities.
- **F3 (MINOR) → §2:** clarified recipient and path share the `server.rs:1317` mint call site
  (distinguished by the intent-variant match at `:1294-1300`); the role must be selected in that
  match and threaded, never hardcoded at the call site.
- **F4 (MINOR, load-bearing) → §3:** pinned the table-construction invariant — an untrusted-origin
  role may appear in a slot's expected-role list only if that slot is also I2-sensitive.
- **F5 (NIT) → §1:** corrected the `TaintLabel` variant list (eight, incl. `PdfRaw`/`LlmGenerated`).
- **F6 (NIT) → §1:** Phase 24 should add `#[serde(default)]` to `origin_role`.

No finding was resolved by weakening a ruling or by designing a locked-out mechanism. The
load-bearing rulings (additive tag, hard-`Denied` Step-1c ordering, `Concat`→`recipient`,
fail-closed `None`-vs-`Some(&[])` contract) survived remediation unchanged.
