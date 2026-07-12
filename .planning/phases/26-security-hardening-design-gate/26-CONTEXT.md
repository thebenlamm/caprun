# Phase 26: Security Hardening Design Gate - Context

**Gathered:** 2026-07-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Author **one** design doc — `planning-docs/DESIGN-security-hardening.md` — that pins
the **mechanism + fail-closed default** for all five v1.6 TCB-local residuals
(HARDEN-01..05), then clear a **fresh, non-self adversarial review** (DESIGN-12) with
every finding resolved, recorded in a GATE-RECORD, **before any `crates/executor`,
`crates/brokerd`, or `crates/runtime-core` hardening code is written**. This is a HARD
gate — it hard-blocks Phases 27–29.

The deliverable is a **decisions doc, not an options survey** — mirror the shape of
`planning-docs/DESIGN-slot-type-binding.md` (v1.5): §-per-mechanism, an
"Adversarial-Review Preemption" section, an "Accepted Residual Risks" section, and
post-review amendments folded into the relevant § after DESIGN-12.

**In scope:** the design doc + its adversarial-review clearance for the five residuals
and the three cross-cutting rulings below.
**Out of scope:** any hardening code (Phases 27–30), and the v1.7 adapter breadth.
</domain>

<decisions>
## Implementation Decisions

These are the mechanism directions the DESIGN doc must pin. They were locked in
discussion (owner ruling on the one scope fork; the rest are recommended fail-closed
rulings that will still face the DESIGN-12 adversarial review). A fresh Fable-5 reviewer
traced the code and surfaced the cross-cutting rulings (§ "Cross-Cutting") that none of
the five residuals individually names — the design doc MUST resolve those too.

### HARDEN-01 — demote-at-RequestFd (I1 honest scope) → §a
- **D-01 (principle, locked):** *Worker-reported evidence may only DEMOTE a session,
  never keep it Active; any "keep-Active" decision must be BROKER-derived.* Demotion
  commits **before** the fd is released to the worker (fail-closed ordering).
- **D-02 (pinned-doc reconciliation, locked):** The pinned clause
  `DESIGN-session-trust-state.md:80-81` ("No other function in brokerd MUST be permitted
  to set `SessionStatus::Draft` for the I1 reason") is **reconcilable, not blocking** —
  the status quo actually *violates its own anti-self-declaration rationale*, because
  `mint_from_read` (the only demotion site today) is reached solely via the
  worker-optional `ReportClaims` path, so a silent/injected worker skips demotion
  entirely. Relocating the demotion to fd-grant is the **broker's own act**, which
  strengthens the spirit. Precedent already exists: `fd_requested` is flipped broker-side
  at RequestFd entry today (`server.rs:1001`). The design doc must state this reconciliation
  explicitly and amend the pinned doc's letter to name RequestFd as a broker-side demotion
  site.
- **D-03 (CONTROL-01 clean path, locked):** The benign fragment-free doc read must still
  stay Active and send. Its stay-Active criterion **shifts from "fragment-free" to
  "trusted-labeled file"** — the design doc must specify where the trusted label at
  fd-grant time comes from (broker-derived), and the **fail-closed default for an
  unlabeled file is demote**. Avoid dragging content-parsing into the TCB at fd time
  (TOCTOU risk) — the label must be broker-derivable without parsing worker-supplied bytes.

### HARDEN-02 — authenticated audit chain (verify_chain) → §b
- **D-04 (threat ceiling, OWNER RULING = "In-host DB-writer"):** v1.6 defends against an
  actor with **`events`-table write access on the host** — NOT a full host/root
  compromise. Mechanism: a **keyed MAC** over the chain with the **key held by the broker,
  outside the confined worker's Landlock filesystem scope**, so a bare DB-file writer
  cannot derive it. Add an **anchored / monotonic chain head** to catch tail-truncation
  and restore-from-backup rollback (a per-entry MAC alone does NOT detect either).
- **D-05 (deferred, named residual):** A full host/root compromise that can read the
  broker's key is **explicitly out of scope for v1.6** and must be recorded as a named
  Accepted Residual Risk (candidate: external out-of-store notarization in a later
  milestone). Do NOT let the doc claim tamper-evidence beyond the in-host-DB-writer model.
- **D-06 (mutable-field trap, locked):** The current audit design keeps one
  field mutable/redactable *outside* the hash. Under a MAC, that field would be wholly
  unauthenticated — the design doc must either accept that explicitly or fold a salted
  commitment of it inside the MAC'd payload. (Verify against `audit.rs` before pinning.)

### HARDEN-03 — Allowed-path replay CAS → §c (largely mechanical)
- **D-07 (mechanism, locked):** An idempotency key / compare-and-swap on the trusted
  (Allowed) `email.send` path, in the **same atomic-transaction discipline as the confirm
  path's PendingConfirmation** at-most-once.
- **D-08 (scope caveat — MUST be in the doc, locked):** CAS buys **at-most-once PER PLAN
  NODE**, NOT bounded sends per session. A statically-compromised worker that mints fresh,
  distinct plan nodes still sends N times — no plan-layer dedup defeats that. The doc must
  state this limit and file an **effects-budget / rate-limit** as named future
  defense-in-depth (not v1.6 scope). Pin what the idempotency key is derived from
  (plan-node hash vs client nonce) during research.

### HARDEN-04 — compile-out the forced-Active mint → §d (largely mechanical)
- **D-09 (mechanism, locked):** Replace the runtime `CAPRUN_ENABLE_IPC_CREATE_SESSION`
  env gate (`server.rs:918-932`) with a dedicated **Cargo feature** (e.g. `test-fixtures`)
  so the forced-`Active` `CreateSession` arm is **physically absent** from a default
  release build. Note the known complication: `#[cfg(test)]` is NOT set when `brokerd`
  compiles as a dependency of an integration-test binary — which is why the runtime flag
  was chosen originally; the feature-flag approach is what closes it.
- **D-10 (verification-integrity gate, locked):** Because Cargo unifies features, a plain
  `cargo test` builds the lib WITH the feature. The mitigation therefore needs its **own
  negative gate** — a separate **no-feature build asserting the IPC arm is absent** (grep
  of the built artifact / a compile-fail or symbol-absence check) — else it verifies
  nothing. Also confirm Phase 30's live proof runs a **featureless** binary matching the
  shipped release (sharpest test: can CONTROL-01 mint Active via the normal trusted-seed
  path in a no-feature build? If yes, run live proofs featureless and the concern
  dissolves).

### HARDEN-05 — file.create `contents` slot → §e
- **D-11 (mechanism, RECOMMENDED depth taken — owner did not object):** Give `contents`
  a **content-sensitive I2 treatment** (mirror email `body`), so a tainted value routed
  into it is blocked/confirmed under the same I2 / slot-type discipline as other sensitive
  args. Today it is `None` (unconstrained) at `sink_sensitivity.rs:157`. Pin the
  expected-role list carefully to **avoid false-positive blocking of legitimate derived
  file writes** (existing trusted-content flows must still succeed — ROADMAP SC-3).
- **D-12 (laundering residual, tracked as cross-cutting):** The *real* deeper risk is a
  **write→re-read laundering round-trip** (worker writes tainted `contents` to a workspace
  file, then re-reads it via RequestFd with provenance evaporated). Full output-file
  provenance labeling would close it directly but is a bigger, adapter-spanning change.
  For v1.6 the input-role treatment is the scoped fix; the laundering loop is **closed by
  the label-continuity ruling (X-01) below** and recorded as a tracked residual.

### Cross-Cutting rulings (the design doc MUST pin these — no single residual names them)
- **X-01 (label continuity):** Broker-written files must carry the **writing session's
  taint label**, or demote-at-RequestFd (§a) is bypassable via the (§e) laundering loop.
  ONE label-provenance model answers both HARDEN-01 and HARDEN-05.
- **X-02 (shared-store restart authority):** Session status, taint labels,
  PendingConfirmation rows, and the new CAS rows all live in the **same SQLite store**
  beside the audit chain — a DB-writer can flip `SessionStatus` or delete a CAS row, and
  the change goes live at broker restart. The doc must pin ONE rule: which security state
  is authoritative **in-memory vs recovered from disk**, and on restart recovered state is
  either **MAC-re-verified** (ties to HARDEN-02) or **fail-closed re-derived** (e.g. all
  recovered sessions resume `Draft`).
- **X-03 (TOCTOU / atomic ordering):** A demotion racing an in-flight Allowed
  `SubmitPlanNode` must be safe — pin ONE uniform atomicity rule: **the status transition
  commits before effect dispatch** (same discipline as demote-before-fd-release in D-01).

### Claude's Discretion
- The exact idempotency-key derivation (D-08), the precise `contents` expected-role
  membership (D-11), and the no-feature negative-gate implementation shape (D-10) are for
  the researcher/planner to pin against live code — the DIRECTIONS above are locked.

### Folded Todos
- **`2026-07-08-v1.3-phase16-v2-security-obligations.md`** (`resolves_phase: 30`) — the
  panel-recorded v2 security obligations. Its items #1–#4 are the source detail for
  residuals (a)–(d): #1 Demote-at-RequestFd → HARDEN-01/D-01..03; #2 verify_chain auth →
  HARDEN-02/D-04..06; #3 Allowed-path replay CAS → HARDEN-03/D-07..08; #4 forced-Active
  compile-exclusion → HARDEN-04/D-09..10. Item #5 (kind-aware Source label in confirm
  narration) is a UX enhancement, **NOT** folded — remains deferred.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements / roadmap (what the gate must satisfy)
- `.planning/REQUIREMENTS.md` — DESIGN-11, DESIGN-12, HARDEN-01..06 (the v1.6 milestone).
- `.planning/ROADMAP.md` §"Phase 26" — the three success criteria (doc pins all five
  mechanisms+defaults; clears a fresh non-self review recorded in a GATE-RECORD; no
  hardening code exists yet).
- `.planning/todos/pending/2026-07-08-v1.3-phase16-v2-security-obligations.md` — folded;
  source detail for residuals (a)–(d).

### Shape to mirror (LOCKED format precedent)
- `planning-docs/DESIGN-slot-type-binding.md` — the v1.5 design-gate doc; mirror its
  §-per-mechanism structure, Adversarial-Review-Preemption §, Accepted-Residuals §, and
  the "pins decisions not options" discipline. Its GATE-RECORD companion:
  `planning-docs/DESIGN-GATE-RECORD-v1.5.md` (the review-record format to reproduce).
- `planning-docs/PLAN.md` — canonical; wins on any conflict.

### Pinned prior designs the doc must reconcile with / amend
- `planning-docs/DESIGN-session-trust-state.md` §:80-81 — the "No other function in
  brokerd MUST set `SessionStatus::Draft` for the I1 reason" clause that D-02 reconciles
  and amends. Read its :84-87 anti-self-declaration rationale — it is the argument that
  the status quo already violates.

### Code surfaces the five residuals touch (verified this session)
- `crates/brokerd/src/server.rs` — `RequestFd` arm (`:996-1001`, `fd_requested=true` at
  entry), the `mint_from_read` demotion path, and the `CreateSession` forced-Active arm
  behind `CAPRUN_ENABLE_IPC_CREATE_SESSION` (`:904-984`, gate at `:918-932`).
- `crates/brokerd/src/audit.rs` — the SHA-256 hash-chain: formula at `:8`,
  `compute_event_hash` (`:243`), the append path (`:277-310`), `verify_chain` (`:469`),
  `event_hash_by_id` (`:227`). `hash`/`parent_hash` are plain TEXT columns (`:41`).
- `crates/brokerd/src/quarantine.rs` — `mint_from_read` (sole I1 demotion site today).
- `crates/executor/src/sink_sensitivity.rs` — `is_content_sensitive` (`:102`),
  `expected_role` (`:147`), `"contents" => None` at `:157` (the exact line HARDEN-05
  changes). `check-invariants.sh` protects the no-wildcard / mint-call-site discipline.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`expected_role()` already exists** (`sink_sensitivity.rs:147`) with the exact
  `Option<&'static [&'static str]>` contract from v1.5 — HARDEN-05 is a table-entry change
  at `:157` (`"contents" => None` → a role list), NOT a new mechanism. Mirror the v1.5
  fail-closed `None`-vs-`Some(&[])` contract.
- **`fd_requested` broker-side flag at RequestFd entry** (`server.rs:1001`) — precedent
  that the broker already mutates per-connection state at fd-grant; HARDEN-01's demotion
  hooks the same site.
- **Confirm-path PendingConfirmation transaction** — the at-most-once discipline
  HARDEN-03's Allowed-path CAS must mirror.
- **`compute_event_hash` / `verify_chain`** — HARDEN-02 wraps these with a keyed MAC +
  anchored head rather than replacing them.

### Established Patterns
- **Hardcoded-in-TCB, no config file** — `sink_sensitivity.rs` is doc-commented "a
  security property, not a configuration knob. CON-i2-non-bypassable." All five mechanisms
  stay hardcoded Rust; NO swappable policy file (I2/I1/I0 stay in the TCB).
- **Exhaustive no-wildcard matches** guarded by `check-invariants.sh` — any new enum
  variant (e.g. a DenyReason or SessionStatus change) forces compile-time updates at all
  match sites; treat the build as the backstop.
- **`EffectRequest` token is build-forbidden under `crates/`** (Gate 1) — keep every new
  lookup a `(SinkId, arg) -> …` shape, never a raw args-map-to-sink path.

### Integration Points
- Phase 27 lands HARDEN-01 + HARDEN-04 in `server.rs`'s session/connection lifecycle.
- Phase 28 lands HARDEN-02 in `audit.rs`.
- Phase 29 lands HARDEN-03 + HARDEN-05 at the sink-dispatch level.
- Phase 30 re-runs the full workspace regression on real Linux via
  `scripts/mailpit-verify.sh` (bare recipe) with new negative tests per closed residual.

</code_context>

<specifics>
## Specific Ideas

- The DESIGN doc must read as **"pins decisions, not options"** — Phase 27–29 are
  mechanical realizations of what §a–§e fix. Every file:line claim must trace to a direct
  code read (re-verify at Phase 27+ if many commits intervene), exactly as the v1.5 doc
  demands.
- DESIGN-12's review must be **genuinely non-self**: a fresh reviewer agent that TRACES
  THE CODE and re-runs any grep — not a self-read. Advisor tool is unavailable this
  session; use an `Agent(model:"fable")` reviewer (standing fallback, has caught real
  blockers). Record the review + resolutions in a `DESIGN-GATE-RECORD-v1.6.md`, mirroring
  the v1.5 GATE-RECORD.
- Standing close-gate disciplines to carry to Phase 30 (not this phase, but pin in the
  doc's proof section): capture `$?` BEFORE any pipe; assert on the PASSED sentinel +
  named test counts, never exit-0-through-a-pipe.

</specifics>

<deferred>
## Deferred Ideas

- **Effects-budget / per-session send rate-limit** — the defense-in-depth beyond HARDEN-03
  CAS that would bound a statically-compromised worker's total sends (D-08). Named future
  work, out of v1.6 scope.
- **External out-of-store notarization for the audit chain** — the host/root-compromise
  defense beyond the in-host-DB-writer ceiling (D-05). Named residual, later milestone.
- **Full output-file provenance labeling model** for `file.create` `contents` (D-12) — the
  deeper laundering-loop fix if the input-role treatment proves insufficient; v1.6 uses the
  scoped input-role fix + X-01 label continuity instead.
- **v1.7 — Breadth:** Git/GitHub/test/patch-PR/snapshot adapters (REQUIREMENTS.md v2).

### Reviewed Todos (not folded)
- `2026-07-07-gsd-phases-clear-deletes-all-milestones.md` — GSD tooling bug; not a caprun
  security residual. Not folded (already tracked as a known GSD gotcha in memory).
- `2026-07-08-gsd-executors-must-not-write-phase-completion-state.md` — GSD process
  discipline (executors must not write ROADMAP/STATE); a standing rule already honored, not
  a Phase 26 deliverable. Not folded.

</deferred>

---

*Phase: 26-security-hardening-design-gate*
*Context gathered: 2026-07-12*
