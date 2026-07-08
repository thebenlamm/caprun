# Phase 15: Deterministic Doc→Action Extraction - Research

**Researched:** 2026-07-08
**Domain:** Rust TCB extension — confined-worker deterministic extraction, multi-input provenance-threading in an audit DAG, anti-stapling verification
**Confidence:** HIGH (current source read directly, file:line cited throughout; no CONTEXT.md exists for this phase — no upstream user constraints to reconcile)

No `.planning/phases/15-*/15-CONTEXT.md` exists (this phase has not been through `/gsd-discuss-phase` yet). There is therefore no `## User Constraints` section — the planner's binding constraints for this phase are `CLAUDE.md`, `.planning/REQUIREMENTS.md` (EXTRACT-01/02/03, CONFIRM-02), and the two APPROVED v1.3 design docs cited throughout this document.

## Summary

Phase 15's actual engineering content is narrower than "build an extractor" and harder than it looks: most of the *scaffolding* for confined-worker deterministic extraction already exists and is reused unmodified (the worker already extracts typed claims from hostile bytes at `cli/caprun/src/worker.rs:149-165`, and the broker already has the sole taint-mint site at `crates/brokerd/src/quarantine.rs:204` `mint_from_read`). What does NOT exist today, and is this phase's real work, is: (1) a **second, live-path arg** — the planner (`cli/caprun/src/planner.rs:58-69`) currently builds `email.send` plan nodes with `to` ONLY; there is no `subject`/`body` arg on the live path at all, even though `crates/executor/src/sink_sensitivity.rs:78` and `crates/brokerd/src/sinks/email_smtp.rs:137-140` both already require/expect them; (2) a **transform-derived mint constructor** that does not yet exist — `mint_from_read` (`quarantine.rs:204-296`) mints exactly one fresh-rooted `ValueRecord` per single input claim, and there is no function anywhere in the codebase that takes MULTIPLE already-tainted inputs and mints a derived value whose `provenance_chain` threads back to all of them (this is the literal gap `DESIGN-confirm-binding.md`'s "Provenance-Threading for Transform-Derived Mints" section names as the closed BLOCKER); and (3) a **generalized, per-blocked-arg unbroken-edge audit query with an anti-staple check** — the closest existing analog (`crates/brokerd/tests/durable_anchor.rs:171-244`) asserts genuine taint for exactly ONE anchor and does not distinguish a genuinely-threaded multi-input chain from a fabricated one; EXTRACT-02 requires this to hold independently for every element of a `Vec<BlockedArg>` (Phase 14's plural shape, `crates/runtime-core/src/executor_decision.rs:147-158`).

**Primary recommendation:** Do not modify `mint_from_read`. Add a new, sibling broker function (call it `mint_from_derivation`, defined in `crates/brokerd/src/quarantine.rs` next to `mint_from_read`/`mint_from_intent`, following the SAME "append event + mint record in one call" atomicity discipline) that accepts `&[&ValueRecord]` (the ALREADY-minted, already-tainted input records — themselves each individually produced by an ordinary `mint_from_read` call per raw field) plus the already-transformed literal, and constructs the derived `ValueRecord` with `taint` = union of inputs' taint, and `provenance_chain` = the concatenation of every input's `provenance_chain` (deduplicated, order-stable), PLUS appends a durable `derivation` Event whose `parent_id` set covers every input read Event (the causal DAG in `Event.parent_id` only supports ONE parent per event today — `crates/runtime-core/src/event.rs:21` `parent_id: Option<Uuid>` — so the derivation edges to MULTIPLE inputs must ride in the `provenance_chain` value-lineage graph, not `parent_id`, matching this project's own documented two-graphs-never-share-edges rule). EXTRACT-02's audit query then walks each `SinkBlockedAnchor.provenance_chain` element-by-element (not just `[0]`), asserting each entry exists as a real DAG event whose `event_type == "file_read"` — EVERY element MUST be a `file_read`; a `derivation` event appearing as a `provenance_chain` element is a fail-closed error, NEVER walked recursively as a chain element (locked two-graphs decision, finding #10; the genuine-derivation edge is a SEPARATE payload predicate over the `derivation` Event's hashed `derived_value_id`/`input_provenance_chains`, per finding #2 — see Pitfall 4). The anti-staple check is the negative-space assertion that a chain rooted at a non-`file_read` event, or at an event with no corresponding claim, is REJECTED as a mint error, mirroring the existing empty-provenance guard (`crates/executor/src/value_store.rs:70-72`).

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Reading hostile doc bytes via granted fd | Confined Worker (`cli/caprun/src/worker.rs`) | Broker (adapter-fs `RequestFd`) | Broker only opens+passes the fd (`adapter-fs`); the worker does the actual read (`worker.rs:141-146`) — unchanged, existing pattern |
| Deterministic field/claim extraction (identify recipient token, body token) over raw bytes | Confined Worker | — | EXTRACT-01 hard requirement: "runs CONFINED... over hostile bytes" — a parser over hostile bytes is attack surface; MUST NOT move into brokerd |
| Transform application (concat, base64-decode) on already-extracted raw fields | Confined Worker | — | `DESIGN-confirm-binding.md` "Post-Transformation Bytes" section: the transform runs before crossing into a mint call; doing it broker-side would put hostile-byte manipulation in the control plane, same violation EXTRACT-01 forbids |
| ValueRecord minting (taint assignment, provenance_chain construction) | Broker (`crates/brokerd/src/quarantine.rs`) | — | Sole taint-mint-site invariant (T-04-03) — worker NEVER constructs a ValueRecord; worker only sends typed, already-transformed literals + provenance metadata over IPC, broker performs the actual mint |
| Audit DAG event append (file_read, derivation, sink_blocked) | Broker (`crates/brokerd/src/audit.rs`) | — | Broker owns the SQLite connection; worker has no DB access post-confinement |
| I2 sensitivity decision (Block on tainted routing/content arg) | Executor (`crates/executor/src/lib.rs`) | — | Unchanged from Phase 14 — the plural collect-then-Block loop already generalizes correctly to N blocked args; Phase 15 does not touch `submit_plan_node`'s decision logic, only what feeds it |
| Unbroken-edge / anti-staple audit query (EXTRACT-02 hard gate) | Test/verification code (new, likely `crates/brokerd/tests/`) | Broker (`audit.rs`, needs a new `find_event_by_id` accessor) | Not a runtime security control — a build-time/test-time PROOF that the runtime's provenance data is genuine; belongs in the test harness, reusing `audit.rs` accessors |
| SMTP send of the confirmed recipient+body | Broker confirm-path process (`crates/brokerd/src/sinks/email_smtp.rs`) | — | Unchanged from Phase 13 — Phase 15 does not touch the adapter; it only ensures `body`/`subject` args exist for it to consume |

## Standard Stack

This phase adds no new *runtime* dependency beyond what is already vendored. It plausibly adds ONE new *direct* Cargo dependency for the base64-decode transform variant of EXTRACT-03.

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `base64` | `0.22.1` | Deterministic, non-LLM base64 decode of an EXTRACT-03 transform-derived value | Already present TRANSITIVELY in `Cargo.lock` (pulled in by `lettre`'s `email-encoding`/`email_address` deps) at exactly this version — adding it as a DIRECT workspace dependency at the same pinned version introduces no new supply-chain surface and avoids hand-rolling a base64 decoder (a deceptively fiddly, security-relevant parsing task: padding, alphabet variants, non-canonical encodings) |

**Version verification:** `base64 0.22.1` confirmed present in the workspace's resolved dependency graph — `grep -A2 'name = "base64"' Cargo.lock` shows `version = "0.22.1"`, `source = registry+https://github.com/rust-lang/crates.io-index` `[VERIFIED: Cargo.lock, local file read]`. Registry legitimacy independently re-checked via the package-legitimacy seam (see Package Legitimacy Audit below) `[VERIFIED: crates.io registry, package-legitimacy seam]`.

**Installation** (only if the base64-decode transform variant of EXTRACT-03 is chosen over the concat-only variant):
```toml
# Add to whichever Cargo.toml hosts the confined-worker-side transform code
# (cli/caprun/Cargo.toml, since the transform MUST run worker-side per EXTRACT-01) —
# pin to the version already resolved in Cargo.lock to avoid a second, divergent
# base64 crate entering the dependency graph.
base64 = "0.22.1"
```

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `sha2`/`hex` (workspace deps, already used) | pinned via `[workspace.dependencies]` (`Cargo.toml:19-20`) | Reused verbatim for the existing `literal_sha256` anti-tamper digest pattern (`crates/executor/src/lib.rs:130-134`) — Phase 15 introduces NO new hash primitive | Any new anchor/digest code this phase touches |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `base64` crate for the base64-decode transform | Hand-rolled base64 decoder | Rejected — base64 decoding has real edge cases (padding, `+/` vs URL-safe alphabet, non-canonical trailing bits) that a hand-rolled decoder gets subtly wrong; this is exactly the class of "deceptively complex, don't hand-roll" problem called out below. The concat transform needs no library at all (plain `String` concatenation is sufficient and involves no parsing) |
| Concat-only EXTRACT-03 fixture | Base64-decode-only fixture | Either alone satisfies EXTRACT-03's "≥1 fixture" bar; concat is strictly simpler (zero new dependencies, zero new failure modes) and is the recommended MINIMUM to satisfy the requirement. Base64-decode is a reasonable SECOND fixture if time allows, since it exercises a genuinely different code path (byte-level transform vs. string-level transform) |

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `base64` | crates.io | ~10 years (published 2015-12-04) | ~20.2M/week | `github.com/marshallpierce/rust-base64` | OK | Approved — add as direct dep only if the base64-decode EXTRACT-03 variant is chosen |

**Packages removed due to [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none.

## Architecture Patterns

### System Architecture Diagram

```
                     ┌─────────────────────────────────────────────────────────┐
                     │              CONFINED WORKER (cli/caprun/src/worker.rs)   │
                     │  (Landlock deny-all + seccomp deny-execve, post-connect)  │
                     │                                                           │
  hostile doc bytes  │  1. read via granted fd (worker.rs:141-146, unchanged)    │
  ───────────────────▶  2. EXTRACT typed fields from raw bytes                   │
  (via fd-pass,       │     [NEW: >1 field per doc — e.g. "local-part" token +   │
   RequestFd,         │      "domain" token for a concat recipient, or a         │
   unchanged)         │      base64 blob token for a decoded body]              │
                     │  3. [NEW] TRANSFORM already-extracted raw fields          │
                     │     (string concat / base64-decode) — over hostile       │
                     │     bytes, confined, BEFORE any IPC send (EXTRACT-01,     │
                     │     D-12(b) "no drift" rule)                              │
                     │  4. Send typed, ALREADY-TRANSFORMED claims + derivation    │
                     │     linkage metadata over IPC (ReportClaims-successor)    │
                     └───────────────────────┬───────────────────────────────────┘
                                             │ IPC (framed JSON, unchanged transport)
                                             ▼
                     ┌─────────────────────────────────────────────────────────┐
                     │                BROKER (crates/brokerd/src/quarantine.rs)  │
                     │                                                           │
                     │  5a. mint_from_read (UNCHANGED, quarantine.rs:204)        │
                     │      — one call per RAW single-source claim               │
                     │      → fresh file_read Event + ValueRecord                 │
                     │        (provenance_chain = [that event's id])             │
                     │                                                           │
                     │  5b. [NEW] mint_from_derivation                            │
                     │      — one call per TRANSFORM-DERIVED claim, taking the    │
                     │        already-minted input ValueRecords as arguments      │
                     │      → derivation Event (parented in provenance_chain,     │
                     │        NOT Event.parent_id — see Pattern 2) +              │
                     │        ValueRecord{ taint: union(inputs), provenance_chain:│
                     │        concat(inputs' chains) }                            │
                     └───────────────────────┬───────────────────────────────────┘
                                             │ ValueId handles only (unchanged handle model)
                                             ▼
                     ┌─────────────────────────────────────────────────────────┐
                     │        EXECUTOR (crates/executor/src/lib.rs, UNCHANGED)   │
                     │  Collect-then-Block loop (Phase 14, already plural) —     │
                     │  Phase 15 adds NO new executor logic. `to`/`body` both     │
                     │  resolve to tainted records → both collected into one     │
                     │  BlockedPendingConfirmation{ anchors: Vec<BlockedArg> }    │
                     └───────────────────────┬───────────────────────────────────┘
                                             │
                                             ▼
                     ┌─────────────────────────────────────────────────────────┐
                     │   EXTRACT-02 VERIFICATION (new test code, brokerd/tests/) │
                     │  For EVERY anchor in anchors: walk provenance_chain,      │
                     │  assert each id resolves to a REAL DAG event, and that    │
                     │  chain (possibly via a derivation event) terminates at a  │
                     │  genuine file_read event — anti-staple check rejects a    │
                     │  chain rooted at a fabricated/unrelated event.            │
                     └─────────────────────────────────────────────────────────┘
```

### Recommended Project Structure

No new crates or top-level modules — extend existing files in place:
```
crates/runtime-core/src/
├── plan_node.rs          # TaintLabel — WorkerExtracted already declared, unused (see Pitfall 1)
crates/brokerd/src/
├── quarantine.rs         # ADD mint_from_derivation() beside mint_from_read/mint_from_intent
├── proto.rs              # EXTEND WorkerClaim (or add a new IPC message) to carry a
│                          # transformed claim + its input-claim linkage
├── server.rs             # EXTEND the ReportClaims dispatch arm (or add a new arm) to
│                          # call mint_from_derivation for transform-derived claims
├── audit.rs              # ADD find_event_by_id(conn, session_id, id) — does not exist
│                          # today (find_event_by_type only returns the FIRST of a type,
│                          # insufficient once >1 file_read/derivation event exists per session)
cli/caprun/src/
├── worker.rs              # EXTEND the extraction step: >1 field, transform application
├── planner.rs             # EXTEND plan_from_intent's email.send arm to emit subject+body
│                          # PlanArgs (currently `to` ONLY — see Pitfall 3)
crates/brokerd/tests/
├── extract_provenance_threading.rs  # NEW — the EXTRACT-02 per-arg unbroken-edge +
│                                    # anti-staple proof, modeled on durable_anchor.rs
└── (doc fixture)                    # NEW — CONFIRM-02's realistic hostile-doc fixture
```

### Pattern 1: Confined-worker multi-field extraction (extends the existing single-claim pattern)

**What:** The existing `extract_email_claims`/`extract_relative_path_claims` functions (`crates/brokerd/src/quarantine.rs:54-119`, imported and called from the CONFINED worker at `cli/caprun/src/worker.rs:45,157-164` despite being defined in the `brokerd` crate) are deterministic, hand-rolled, dependency-free word scanners. Phase 15 needs a scanner that can find MULTIPLE distinct fields in one document (e.g., a "local-part" token and a separate "domain" token to concat into a recipient, or a base64 blob token for the body) — this is a natural EXTENSION of the existing pattern, not a new architecture.

**When to use:** Any time the confined worker needs to identify >1 semantically-distinct hostile substring in one read.

**Example (existing pattern to extend, cited verbatim):**
```rust
// Source: crates/brokerd/src/quarantine.rs:54-74 (existing, current code)
pub fn extract_email_claims(raw: &str) -> Vec<Claim> {
    let mut claims = Vec::new();
    for word in raw.split_whitespace() {
        let trimmed = word.trim_matches(|c: char| {
            !c.is_alphanumeric() && c != '@' && c != '-' && c != '_' && c != '+'
        });
        if looks_like_email(trimmed) {
            claims.push(Claim { claim_type: "email_address".into(), value: trimmed.to_string() });
        }
    }
    claims
}
```
A Phase-15 field-token extractor (e.g. for a concat-recipient's two halves, or a body's base64 blob) should follow this exact shape: pure function, no regex crate, no LLM, returns a typed `Vec<Claim>`-like structure. **Do not introduce a regex dependency** — the existing pattern is deliberately hand-rolled per this file's own doc comment discipline, and every existing extractor in this codebase follows it.

### Pattern 2: Two-graphs-never-share-edges (the model for multi-parent provenance)

**What:** This codebase has an EXPLICIT, already-recorded architectural decision (found in `mint_from_read`'s own doc comment, `quarantine.rs:192-201`, and exercised by `crates/brokerd/tests/durable_anchor.rs:19-23`): the causal DAG (`Event.parent_id`, walked by `verify_chain`) and the value-lineage graph (`ValueRecord.provenance_chain`/`SinkBlockedAnchor.read_event_id`) are TWO DISTINCT GRAPHS that happen to share node ids but NEVER share edges. `Event.parent_id: Option<Uuid>` (`crates/runtime-core/src/event.rs:21`) supports exactly ONE causal parent — it structurally CANNOT represent "this derivation event has two input reads as parents." This is fine, because the DESIGN doc's provenance-threading requirement (`DESIGN-confirm-binding.md` "Provenance-Threading for Transform-Derived Mints") does not require a multi-parent `parent_id` — it requires the *value-lineage* graph (`provenance_chain`, a `Vec<Uuid>`) to carry the derivation edges. Use the SAME pattern: `provenance_chain` becomes the vehicle for "this derived value's ancestry passes through these N read events," while `parent_id` stays single-parent and continues to model only the CAUSAL/temporal event ordering (e.g., the new `derivation` event's `parent_id` = the connection's chain head, same as every other event append today).

**When to use:** Any time Phase 15 needs to express "this value has more than one tainted ancestor."

**Example (illustrative — this is new code, not existing code to cite verbatim):**
```rust
// Illustrative shape for a NEW crates/brokerd/src/quarantine.rs function,
// ⚠ SUPERSEDED SKETCH — read 15-01-PLAN.md Task 2 for the authoritative shape.
// This illustrative sketch predates plan-review findings #2/#3/#10 and is now
// INCOMPLETE. The executable spec (15-01 Task 2) additionally requires:
//   • the derivation Event is built via Event::derivation (NOT Event::new), carrying
//     the hashed payload derived_value_id/input_value_ids/input_provenance_chains/
//     transform_kind (finding #2) — the sketch's Event::new omits these;
//   • the taint union DROPS UserTrusted when any untrusted label is present (finding #3);
//   • a file_read-root guard: when the union is_untrusted, provenance_chain[0] MUST
//     resolve (session-scoped) to a file_read event, else fail closed (finding #3);
//   • a transform_kind parameter.
// The sketch below is retained only to show the atomicity/dedup shape.
//
// modeled directly on mint_from_read's existing atomicity discipline
// (append event + mint record in ONE call, quarantine.rs:204-296).
pub fn mint_from_derivation(
    conn: &rusqlite::Connection,
    store: &mut ValueStore,
    session_id: Uuid,
    transformed_literal: String,
    inputs: &[&ValueRecord],   // the ALREADY-minted input records (e.g. two mint_from_read outputs)
    parent_id: Option<Uuid>,   // CAUSAL parent (chain head) — unrelated to `inputs`
    parent_hash: Option<&str>,
) -> Result<(Uuid, String, ValueId, /* ...chain-head plumbing mirroring mint_from_read... */)> {
    // Fail-closed: a "derivation" with zero inputs is a contradiction —
    // reject rather than silently falling back to a fresh-rooted mint
    // (mirrors the existing empty-taint/empty-provenance guards in
    // executor/src/value_store.rs:67-72).
    if inputs.is_empty() {
        return Err(anyhow::anyhow!(
            "mint_from_derivation: zero inputs — a derived value must thread \
             at least one input's provenance (fail-closed, never fresh-root)"
        ));
    }

    // taint = union of every input's taint (monotonic, never narrowed — D-16 point 1).
    let mut taint: Vec<TaintLabel> = Vec::new();
    for r in inputs {
        for t in &r.taint {
            if !taint.contains(t) { taint.push(t.clone()); }
        }
    }

    // provenance_chain = concatenation of every input's chain, in input order,
    // deduplicated. Root (index 0) is therefore one of the ORIGINATING reads —
    // never a fresh transform-local event (D-16 point 2/the anti-laundering rule).
    let mut provenance_chain: Vec<Uuid> = Vec::new();
    for r in inputs {
        for id in &r.provenance_chain {
            if !provenance_chain.contains(id) { provenance_chain.push(*id); }
        }
    }

    // Append a durable `derivation` Event so the audit DAG carries the
    // transform as a real event too (D-16 point 3) — its CAUSAL parent is the
    // connection chain head (unrelated to `inputs`); its role in the
    // VALUE-lineage graph is carried entirely by `provenance_chain` above,
    // per Pattern 2 (two graphs, never shared edges).
    let derivation_event_id = Uuid::new_v4();
    let derivation_event = Event::new(
        derivation_event_id, parent_id, session_id,
        "confined-reader".into(), "derivation".into(), Utc::now(), taint.clone(),
    );
    let derivation_hash = append_event(conn, &derivation_event, parent_hash)?;

    // Mint fails closed if the invariants above somehow produced an empty
    // taint/chain (defense in depth — should be unreachable given the guard above).
    let value_id = store.mint(transformed_literal, taint, provenance_chain)
        .map_err(|e| anyhow::anyhow!("mint invariant: {e:?}"))?;

    Ok((derivation_event_id, derivation_hash, value_id /* , ...chain-head plumbing... */))
}
```

### Anti-Patterns to Avoid

- **Re-using `mint_from_read`'s single-input signature for a transform:** `mint_from_read` takes exactly one `Claim` and ALWAYS mints `provenance_chain = vec![fresh_event_id]` rooted in an event it appends in that same call (`quarantine.rs:244-268`). Calling it a second time on an already-transformed string (e.g., `mint_from_read(concat(a,b))`) produces a value whose `provenance_chain` roots at a BRAND NEW, unrelated event — the exact laundering failure `DESIGN-confirm-binding.md`'s "Provenance-Threading" section calls a "closed BLOCKER." This is the single most important anti-pattern this phase must not fall into.
- **Doing the transform in the broker instead of the worker:** Base64-decoding or concatenating raw doc bytes in `brokerd` (rather than in the confined `cli/caprun/src/worker.rs`) would put hostile-byte parsing in the control plane — the exact thing EXTRACT-01 forbids ("a parser over hostile bytes is attack surface... not in the broker control plane").
- **Resolving a `ValueId` to its literal, transforming it, then reusing the SAME `ValueId`/anchor as if untransformed:** explicitly named as forbidden in `DESIGN-confirm-binding.md`'s "Post-Transformation Bytes" section — any transform MUST mint a FRESH `ValueRecord` for the transformed value before it is ever used as a plan-node arg.
- **Using `find_event_by_type` for EXTRACT-02's per-anchor lookup:** `find_event_by_type` (`crates/brokerd/src/audit.rs:299-320`) returns only the FIRST event of a given `event_type` per session (`LIMIT 1`, line 308). Once >1 `file_read`/`derivation` event exists per session (which multi-field extraction guarantees), this function cannot disambiguate WHICH `file_read` a given anchor's `provenance_chain[0]` refers to. A new `find_event_by_id(conn, session_id, id)` accessor is required (trivial `WHERE id = ?1` variant of the same query shape) — do not force-fit `find_event_by_type` into this role.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Base64 decoding of an EXTRACT-03 transform input | A hand-rolled base64 decoder | the `base64` crate (already transitively pinned at 0.22.1, see Standard Stack) | Padding/alphabet/non-canonical-encoding edge cases are a well-known source of parser bugs; this is exactly the kind of "deceptively complex" problem the project's own `mac-guardian`/security discipline exists to avoid re-litigating per phase |
| A second, parallel hash-chained "derivation graph" data structure | A brand-new schema/table for derivation edges | `ValueRecord.provenance_chain: Vec<Uuid>` (already exists, already a `Vec`, already the load-bearing anti-stapling field) + a plain `derivation` `Event.event_type` string (no schema migration — `Event` already stores an arbitrary `event_type: String`, `event.rs:25`) | The codebase already has exactly the extensibility point needed (a `Vec<Uuid>` chain and a free-form event-type string) — inventing a new table/relation would duplicate `provenance_chain`'s job and risk the "two independent, divergent definitions of provenance" bug class this project's design docs repeatedly warn against (e.g. `DESIGN-confirm-binding.md`'s finding #4 "producer hashing H(body) while a verifier hashes H(to‖body)") |
| A regex-based doc field extractor | `regex` crate | the existing hand-rolled `split_whitespace` + `trim_matches` + structural-shape-check pattern (`quarantine.rs:54-145`) | Every existing extractor in this codebase is deliberately regex-free and hand-rolled (stated design choice, not an oversight) — introducing `regex` for Phase 15 alone breaks that consistency and adds an unreviewed dependency for a problem the existing pattern already solves at this project's extraction complexity level |

**Key insight:** Phase 15's hardest problem (provenance-threading) is NOT a "use a library" problem — it is a data-modeling problem internal to this codebase's own `ValueRecord`/`Event` types. The two "don't hand-roll" items above (base64, regex-avoidance) are real but secondary; the primary risk is architectural (re-inventing a provenance model) rather than a missing-library risk.

## Runtime State Inventory

Not applicable — this is a greenfield feature-addition phase (new extraction/mint capability), not a rename/refactor/migration. No existing stored data, service config, OS-registered state, secrets, or build artifacts reference a name being changed.

## Common Pitfalls

### Pitfall 1: Assuming `TaintLabel::WorkerExtracted` already has defined semantics
**What goes wrong:** A plan might assume `WorkerExtracted` (declared in `crates/runtime-core/src/plan_node.rs:20`, `is_untrusted() == true` per line 46) is the "correct" label to use for transform-derived values and start using it without confirming that assumption against any design doc.
**Why it happens:** The label already exists, already returns `true` from `is_untrusted()`, and its name ("Worker Extracted") READS as if it were built for exactly this phase.
**How to avoid:** Note that `WorkerExtracted` is currently MINTED NOWHERE in the codebase `[VERIFIED: grep -rn "WorkerExtracted" across all *.rs files under crates/ and cli/, matches only the enum declaration, the is_untrusted() match arm, confirmation.rs's display-string match arm, and one test asserting is_untrusted()==true — zero mint call sites]`. No design doc (`DESIGN-content-adapter-mediation.md`, `DESIGN-confirm-binding.md`, `DESIGN-taint-model.md`) assigns it a specific meaning for Phase 15's transform-derived values. **This is a real, useful candidate** (its taint-union rule in `mint_from_derivation` above could reasonably ADD `WorkerExtracted` to the union for transform-derived values, marking them as distinct from directly-read raw claims) but this is a NEW design decision this phase would be making, not a pre-approved one — flag it for the planner/discuss-phase as `[ASSUMED]`, not a settled fact.
**Warning signs:** A plan that treats `WorkerExtracted`'s intended use as "obviously already decided" rather than as a phase-15 design choice to make explicit.

### Pitfall 2: Populating `subject`/`body` PlanArgs without updating `plan_from_intent`
**What goes wrong:** Writing a new extraction/mint path but never touching `cli/caprun/src/planner.rs:58-69`'s `SendEmailSummary` arm, which TODAY builds an `email.send` `PlanNode` with `to` as the ONLY arg. `email_smtp.rs::build_message` (`crates/brokerd/src/sinks/email_smtp.rs:134-141`) requires `subject`/`body` to already be present in `resolved_args` or FAILS (`ok_or_else` → `Err`) at confirm/send time — a live run would extract a body value successfully, mint it, get it Blocked by the executor, but then the SMTP adapter would fail closed at send time because the planner never put `subject`/`body` into the plan node's `args` in the first place.
**Why it happens:** The extraction/mint work (worker + quarantine.rs) and the plan-node-shape work (planner.rs) are in different files/crates and easy to treat as separately "done."
**How to avoid:** Any Phase-15 plan MUST include a task updating `plan_from_intent`'s `SendEmailSummary` arm to emit `to`+`subject`+`body` PlanArgs (using the appropriate UserTrusted/tainted ValueIds per the extraction outcome), not just the mint-path plumbing.
**Warning signs:** `cargo test -p executor` (unit-level, hand-constructed PlanNodes) passing while a live worker→broker→executor run for `email.send` never surfaces a body Block at all (because no body arg was ever submitted).

### Pitfall 3: Reusing `find_event_by_type` for EXTRACT-02's verification query
**What goes wrong:** Writing the EXTRACT-02 "per-blocked-arg unbroken edge" test using `find_event_by_type(conn, session_id, "file_read")`, which silently returns only the FIRST `file_read` event in the session (`audit.rs:308`, `LIMIT 1`) — with two extracted fields (recipient half A, recipient half B, or recipient + body), there are now ≥2 `file_read` events per session, and the test would spuriously "pass" by checking the wrong anchor's ancestry against an unrelated event that happens to share the type string.
**Why it happens:** `find_event_by_type` is the only existing lookup helper and the natural first reach.
**How to avoid:** Add `find_event_by_id(conn, session_id, id: Uuid) -> Result<Option<Event>>` (a trivial `WHERE id = ?1 AND session_id = ?2` variant of the exact same query shape as `find_event_by_type`) and use THAT to resolve each `anchor.provenance_chain[i]` to its specific event, for every `i`, for every anchor in `anchors: Vec<BlockedArg>`.
**Warning signs:** An EXTRACT-02 test that only ever calls `find_event_by_type` and never looks up an event BY ITS SPECIFIC UUID.

### Pitfall 4: Building the anti-staple check as "chain is non-empty" instead of "chain root is genuine"
**What goes wrong:** The EXISTING guard (`crates/executor/src/value_store.rs:70-72`, `EmptyProvenance`) already rejects an EMPTY `provenance_chain`. It is tempting to declare EXTRACT-02's "anti-staple check" satisfied by this existing guard alone. It is NOT — a FABRICATED, non-empty, single-element chain rooted at a random fresh `Uuid::new_v4()` (never appended to the DAG as any event) or at a genuine-looking but UNRELATED event (e.g., an `intent_received` event, which carries no untrusted taint) would pass the non-empty check while still being "taint stapled on at the sink" in every meaningful sense.
**Why it happens:** The existing invariant (non-empty chain) is necessary but not sufficient, and it's easy to conflate "an invariant exists" with "the invariant we need exists."
**How to avoid:** The EXTRACT-02 test must POSITIVELY resolve every chain element to a REAL row in the `events` table (via the new `find_event_by_id`) and assert that row's `event_type` is `"file_read"` — EVERY `provenance_chain` element MUST be `event_type == "file_read"`; a `"derivation"` event appearing as a `provenance_chain` element is a fail-closed error (REVISED per plan-review finding #10 — the earlier "or, transitively, a derivation event's chain resolves further back" phrasing is STRUCK: it invited walking a derivation event as a chain element, violating the locked two-graphs decision). The genuine-derivation edge is a SEPARATE predicate over the derivation Event's HASHED PAYLOAD (finding #2): ∃ a `"derivation"` event whose payload `derived_value_id == anchor.value_id` AND `∪input_provenance_chains == anchor.provenance_chain` — checked DB-alone, never by chain-element recursion. The test must ALSO identity-pin each anchor's root vector to the specific minted `file_read` ids (finding #12), and include NEGATIVE controls: (A) a hand-constructed anchor whose `provenance_chain` contains a `Uuid::new_v4()` never appended to the DAG must FAIL; (B) a SAME-SESSION naive re-mint of the concatenated literal must FAIL on the payload-binding predicate, never on a session-wide existence query (finding #11) — a passing check on genuine data proves nothing without a paired failing check on fabricated data, mirroring this project's own `tamper_evidence_mutating_payload_breaks_verify_chain` pattern in `durable_anchor.rs:323-369`, which asserts BOTH a passing baseline AND a failing tampered case.
**Warning signs:** An EXTRACT-02 test file with no test that constructs a deliberately-fabricated/re-anchored chain and asserts REJECTION.

### Pitfall 5: Forgetting `sink_schema.rs`'s `required: &[]` for `email.send`
**What goes wrong:** `crates/executor/src/sink_schema.rs:50-51` currently has `email.send` with `required: &[]` — NO required args at the schema-gate level (Step 0), even though `subject`/`body` are functionally required by the SMTP adapter (Pitfall 2). A plan node missing `body` entirely still passes Step 0's schema gate today (silently), and only fails much later, at `confirm()`-time send, in a way that is NOT audited as a schema violation.
**Why it happens:** `required` was never updated when CONTENT-01 made `subject`/`body` content-sensitive in Phase 14 — Phase 14's own scope was deliberately narrow (I2 decision logic only), not schema completeness.
**How to avoid:** Consider (as a planner decision, not a settled fact — flagging as an open question below) whether Phase 15/16 should tighten `sink_schema.rs`'s `email.send` entry to `required: &["to", "subject", "body"]`, so a malformed/incomplete plan node fails closed at Step 0 (`MissingArg`) rather than reaching the adapter and failing there. This is NOT strictly required by EXTRACT-01/02/03/CONFIRM-02's literal text, but is a natural consistency fix this phase's planner should explicitly decide to include or defer.
**Warning signs:** A live acceptance run (Phase 17) that fails at the SMTP adapter with a generic `build_message missing required 'body' arg` error instead of a clean, audited `Denied(MissingArg)` at Step 0.

## Code Examples

### Existing single-input mint pattern (the analog to extend, NOT modify)
```rust
// Source: crates/brokerd/src/quarantine.rs:204-296 (existing, current code — cited, not modified)
pub fn mint_from_read(
    conn: &rusqlite::Connection,
    store: &mut ValueStore,
    session_id: Uuid,
    claim: &Claim,
    parent_id: Option<Uuid>,
    parent_hash: Option<&str>,
) -> Result<(Uuid, String, runtime_core::plan_node::ValueId, Uuid, String)> {
    // taint derived from claim.claim_type (fail-closed on unknown type);
    // event appended; store.mint(claim.value.clone(), taint, vec![event_id])
    // called with provenance_chain ALWAYS length 1, rooted at the event just
    // appended in THIS SAME call. This is correct for a raw single-field read
    // and MUST NOT be reused for a transform-derived value (see Anti-Patterns).
    ...
}
```

### Existing plural-anchor decision shape (unchanged by Phase 15, consumed as-is)
```rust
// Source: crates/runtime-core/src/executor_decision.rs:147-179 (existing, current code)
pub struct BlockedArg {
    pub anchor: SinkBlockedAnchor,
    pub literal: String,
}
pub enum ExecutorDecision {
    Allowed,
    BlockedPendingConfirmation { anchors: Vec<BlockedArg> },
    Denied { reason: DenyReason },
    NotImplemented,
}
```
Phase 15 needs ZERO changes here — Phase 14 already generalized this to N blocked args. The `collect_then_block_both_to_and_body` test (`crates/executor/tests/executor_decision.rs:437-485`) already proves the executor-side mechanics work for a hand-constructed two-tainted-arg plan node; what it does NOT prove (and what EXTRACT-02 requires) is that each of those two tainted `ValueRecord`s came from a GENUINE, DAG-traceable read/derivation — that test mints both records with `vec![Uuid::new_v4()]` placeholder chains (`executor_decision.rs:444`, `451`), i.e. FABRICATED roots that are never appended to any DAG. This is fine for an executor-logic unit test but is explicitly NOT a valid model for EXTRACT-02's proof — do not mistake it for one.

### Existing genuine-taint durable proof pattern (the model to generalize for EXTRACT-02)
```rust
// Source: crates/brokerd/tests/durable_anchor.rs:205-227 (existing, current code,
// single-anchor case — EXTRACT-02 requires this loop-generalized over
// anchors: Vec<BlockedArg>, plus the negative/fabricated-chain control described
// in Pitfall 4)
assert!(!anchor.provenance_chain.is_empty(), "...");
assert_eq!(anchor.read_event_id, anchor.provenance_chain[0], "...");
let file_read = find_event_by_type(&reopened, &sid, "file_read")
    .expect("query file_read")
    .expect("a file_read event must exist in the reopened DAG");
assert_eq!(file_read.id, anchor.read_event_id, "...");
assert!(file_read.taint.iter().any(|t| t.is_untrusted()), "...");
```

## State of the Art

| Old Approach (Phase 12-14) | Current/Target Approach (Phase 15) | When Changed | Impact |
|--------------|------------------|--------------|--------|
| One `mint_from_read` call per single raw field, `provenance_chain` always length-1 fresh-rooted | ADD `mint_from_derivation` for transform-derived values, threading multi-input `provenance_chain` | This phase | New minting path; `mint_from_read` itself stays byte-for-byte unchanged (do not touch it — the DESIGN doc explicitly calls it "the mint_from_read successor," i.e. an ADDITION, never a modification) |
| `email.send` PlanNode carries `to` only on the live worker/planner path | `planner.rs` must emit `to`+`subject`+`body` | This phase (Pitfall 2) | Without this, Phase 15/16/17's live acceptance cannot exercise the body-block path end-to-end via the real worker, even though the executor/adapter already support it |
| `find_event_by_type` (first-of-type only) | New `find_event_by_id` needed for per-anchor disambiguation | This phase | Required once >1 event of the same `event_type` exists per session — true as soon as multi-field extraction ships |

**Deprecated/outdated:** Nothing in this phase deprecates prior-phase code; Phase 15 is purely additive on top of Phase 12-14's approved, shipped design.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `TaintLabel::WorkerExtracted` is an intentionally pre-staged label meant for transform-derived values in Phase 15 | Pitfall 1 | If wrong (it was simply unused scaffolding with no intended future use), a plan that builds `mint_from_derivation`'s taint-union rule around it would need a design revisit — low risk either way, since the union-of-inputs-taint rule works regardless of which specific labels are unioned; only the CHOICE to additionally tag `WorkerExtracted` is assumption-bearing |
| A2 | A base64-decode transform is the recommended SECOND (optional) EXTRACT-03 fixture variant, concat being the recommended FIRST/minimum | Standard Stack, Alternatives Considered | Low risk — EXTRACT-03 only requires "≥1 fixture," so choosing only the concat variant fully satisfies the requirement; this is a sequencing/scope recommendation, not a load-bearing technical claim |
| A3 | `Event.parent_id`'s single-parent structure is intentional and should NOT be widened to `Vec<Uuid>` to directly model multi-parent derivation edges — `provenance_chain` should carry that instead | Pattern 2 | If a future adversarial reviewer decides multi-parent `parent_id` is actually required, this phase's `mint_from_derivation` design would need revision — however this recommendation is grounded directly in this project's own already-recorded "two graphs, never share edges" design decision (cited from `quarantine.rs`'s own doc comment and exercised by an existing test), not a speculative choice |

## Open Questions

1. **Should `sink_schema.rs`'s `email.send` entry gain `required: &["to", "subject", "body"]`?**
   - What we know: today it is `required: &[]` (Pitfall 5); the SMTP adapter functionally requires all three at send time regardless.
   - What's unclear: whether tightening the schema gate is in-scope for Phase 15 specifically, or should be deferred to Phase 16 (which already owns confirm-UX/negative-control changes) or left as-is with the adapter's existing `Err`-on-missing-arg as the enforcement point.
   - Recommendation: flag for `/gsd-discuss-phase` — it's a small, low-risk, high-value consistency fix, but it touches a hardcoded TCB schema table (`CON-i2-non-bypassable` territory) and should be an explicit decision, not an incidental one.

2. **Exact shape of the new/extended IPC message for transform-derived claims.**
   - What we know: today's `WorkerClaim` (`crates/brokerd/src/proto.rs:22-31`) carries exactly one flat value per claim (an email address or a path string) with no linkage metadata between claims.
   - What's unclear: whether the cleanest wire shape is (a) sending N raw single-field claims via the EXISTING `ReportClaims`/`WorkerClaim` unchanged, then a SEPARATE new `ReportDerivedClaim { transform: TransformKind, input_indices: Vec<usize> }`-shaped message referencing the just-returned `value_ids`, or (b) a single new claim variant that inlines the raw inputs plus a transform tag in one message. Both satisfy EXTRACT-01 (transform runs worker-side); the tradeoff is wire-protocol surface area vs. round-trip count.
   - Recommendation: option (a) is more consistent with the existing "existing types stay as-is; extend additively" discipline this codebase's design docs repeatedly favor (e.g. D-18 "plan-node API untouched," CONFIRM-03 "extends, does not replace `PendingConfirmation`") — recommend the planner default to (a) unless a concrete wiring problem is found.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo`/`rustc` (workspace toolchain) | All Phase 15 code | ✓ | matches workspace `edition = "2021"` | — |
| Colima + Docker (`rust:1` image, `seccomp=unconfined`) | Any Linux-only confinement test Phase 15 adds (unlikely — extraction/mint logic is cross-platform; only a live worker run would need it, and that's Phase 17's job) | ✓ (per `CLAUDE.md`'s documented recipe) | n/a | Phase 15's own EXTRACT-02 proof is DB-alone (SQLite, cross-platform) per the `durable_anchor.rs` model — it does NOT require the Linux sandbox stack; only a live end-to-end worker run would, and that belongs to Phase 17 |
| `base64` crate (crates.io) | EXTRACT-03's optional base64-decode transform variant | ✓ (already resolvable — present in `Cargo.lock` transitively at 0.22.1) | 0.22.1 | Skip the base64-decode fixture variant entirely and satisfy EXTRACT-03 with the concat-only transform (no new dependency needed at all) |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** the `base64` crate has a zero-dependency fallback (use concat-only for EXTRACT-03).

## Validation Architecture

`.planning/config.json` does not set `workflow.nyquist_validation` — absent means enabled, so this section is included.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` (Rust built-in test harness), workspace-wide |
| Config file | none — no `pytest.ini`/`jest.config.*` equivalent; test discovery is Cargo's standard `#[test]`/`tests/*.rs` convention |
| Quick run command | `cargo test -p executor` / `cargo test -p brokerd <new_test_file_stem>` (single crate/target, fast, no Linux/Docker needed for Phase 15's own logic) |
| Full suite command | `cargo test --workspace --no-fail-fast` (per `CLAUDE.md`) — plus `./scripts/check-invariants.sh` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| EXTRACT-01 | Extraction/transform code runs confined-worker-side, never brokerd | structural/unit | `cargo test -p brokerd quarantine` (extraction unit tests) + manual code-location review (no automated "is this confined" test exists or is needed — the invariant is architectural placement, verified the same way `check-invariants.sh` Gate 1/2 verify placement, by grep/code-review, not runtime) | ❌ Wave 0 — extend `quarantine.rs`'s existing test module |
| EXTRACT-02 | Per-blocked-arg unbroken edge + anti-staple check, for EVERY anchor in a multi-arg Block | integration (DB-alone, cross-platform) | `cargo test -p brokerd extract_provenance_threading` (new file) | ❌ Wave 0 — new file, modeled on `durable_anchor.rs` |
| EXTRACT-03 | ≥1 fixture: transform-derived value still propagates taint + still blocks | integration | same new test file, or a sibling — asserts `submit_plan_node` still returns `BlockedPendingConfirmation` for a `mint_from_derivation`-produced value | ❌ Wave 0 |
| CONFIRM-02 | Realistic hostile-doc fixture (embedded injection attempting to redirect/alter the send) exists, reusable across tests + live demo | fixture data + a test asserting extraction finds the expected fields | new fixture file/string constant, consumed by the EXTRACT-02/03 tests above | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p executor` and/or `cargo test -p brokerd <touched_module>` (whichever crate the task touched)
- **Per wave merge:** `cargo test --workspace --no-fail-fast` + `./scripts/check-invariants.sh`
- **Phase gate:** Full suite green, PLUS the new EXTRACT-02 test's negative/anti-staple control (Pitfall 4) must be present and passing — this is the phase's hard, non-negotiable acceptance bar per `ROADMAP.md`'s own text ("phase FAILS if not met").

### Wave 0 Gaps
- [ ] `crates/brokerd/tests/extract_provenance_threading.rs` (or similarly named) — new file covering EXTRACT-02/EXTRACT-03, modeled on `durable_anchor.rs`'s after-exit DB-alone pattern
- [ ] `crates/brokerd/src/audit.rs::find_event_by_id` — new accessor, does not exist today (Pitfall 3)
- [ ] A CONFIRM-02 hostile-doc fixture (string constant or file) — no existing analog beyond the CRLF-injection body string already used in `email_smtp_acceptance.rs:315` and `durable_anchor.rs`'s `HOSTILE_PATH` constant (`durable_anchor.rs:53`); Phase 15's fixture needs to embed enough structure for a multi-field extraction (e.g., two recipient-half tokens, or a body field plus a base64 blob), not just a single email address

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | No auth surface touched this phase |
| V3 Session Management | no | Session-trust-state (I0/I1) is untouched — `mint_from_read`'s existing demotion behavior is reused unmodified for raw single-field claims; `mint_from_derivation` does not itself demote (only `mint_from_read` is the sole I1 trust-flip site per its own doc comment, `quarantine.rs:166-175` — a Phase 15 `mint_from_derivation` MUST NOT duplicate or bypass this, and should rely on its inputs already having been minted via `mint_from_read`, which already demoted the session) |
| V4 Access Control | no | No new authorization boundary — the executor's existing I2 collect-then-Block loop is reused unmodified |
| V5 Input Validation | yes | The existing hand-rolled, dependency-free, deterministic word-scanner pattern (`quarantine.rs:54-119`) — extend it, do not introduce a regex/parser-library attack surface (Don't Hand-Roll) |
| V6 Cryptography | yes (peripheral) | `sha2`/`hex` already-pinned workspace deps for the existing `literal_sha256` digest pattern — reused verbatim, no new crypto primitive introduced |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Taint laundering via a transform that fabricates a fresh provenance root (the core threat this phase exists to close) | Spoofing (of provenance) / Tampering | `mint_from_derivation`'s mandatory input-threading + fail-closed rejection of a derived value with a re-anchored/fresh chain (Pattern 2, Pitfall 4) |
| Confined worker smuggling an untransformed literal as if pre-transformed, to make the digest cover different bytes than what's sent | Tampering | The existing "mint happens ONLY after transform, no transform after mint" rule (`DESIGN-confirm-binding.md` "Post-Transformation Bytes") — not new to this phase, but Phase 15's extractor is the first code that could violate it, so it is a live risk here specifically |
| Extraction logic itself becoming a parser-based attack surface if implemented broker-side | Elevation of Privilege (confined worker → control plane) | EXTRACT-01's confined-worker placement requirement (Architectural Responsibility Map) |
| A hostile doc engineered to make the extractor produce a plausible-looking but WRONG recipient/body (semantic manipulation, not a memory-safety bug) | Tampering (of the effect's meaning, not the runtime's integrity) | Out of scope for this phase's security PROOF — CONFIRM-02's fixture demonstrates the ATTEMPT and the block, but this project's own DOC-01 requirement (Phase 17) explicitly disclaims proving semantic-manipulation-resistance beyond "the human sees the verbatim literal + provenance before confirming" |

## Sources

### Primary (HIGH confidence — direct source read this session)
- `crates/runtime-core/src/plan_node.rs`, `value_record.rs`, `executor_decision.rs`, `event.rs` — full file reads
- `crates/brokerd/src/quarantine.rs`, `server.rs` (relevant ranges), `proto.rs`, `confirmation.rs` (relevant ranges), `audit.rs` (function signatures + relevant bodies), `sinks/email_smtp.rs` — full/targeted reads
- `crates/executor/src/lib.rs`, `value_store.rs`, `sink_sensitivity.rs`, `sink_schema.rs`, `tests/executor_decision.rs` (relevant ranges) — full/targeted reads
- `cli/caprun/src/worker.rs`, `planner.rs` — full reads
- `crates/brokerd/tests/durable_anchor.rs`, `email_smtp_acceptance.rs` — full reads
- `planning-docs/DESIGN-content-adapter-mediation.md`, `DESIGN-confirm-binding.md`, `DESIGN-taint-model.md` (relevant sections) — full/targeted reads
- `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `CLAUDE.md`, `.planning/STATE.md`, `.planning/phases/14-*/14-PATTERNS.md`, `Cargo.toml`, `Cargo.lock` (grep-verified), `scripts/check-invariants.sh`

### Secondary (MEDIUM confidence)
- `gsd-tools query package-legitimacy check --ecosystem crates base64` — registry metadata (publish date, downloads, repo URL) `[VERIFIED: crates.io registry via seam]`

### Tertiary (LOW confidence)
- None — no WebSearch was needed; this phase's research surface is entirely internal to the already-approved design docs and the current source tree.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — only one candidate new dependency (`base64`), already transitively resolved at a known-good version, verified via the package-legitimacy seam
- Architecture: HIGH — every claim about current code shape is a direct, cited source read this session; the NEW-code recommendations (`mint_from_derivation`, `find_event_by_id`) are grounded in explicit gaps found by grep (zero existing call sites / zero existing accessor) and in this project's own already-recorded "two graphs, never share edges" design decision
- Pitfalls: HIGH — each pitfall traces to a specific file:line showing either an existing incomplete wiring (Pitfall 2, 5) or an existing insufficiency in a lookup helper (Pitfall 3) or a genuine ambiguity in existing scaffolding (Pitfall 1)

**Research date:** 2026-07-08
**Valid until:** Effectively pinned to this milestone — this research is valid as long as Phase 12/14's approved design docs and shipped code (`quarantine.rs`, `executor_decision.rs`, `sink_sensitivity.rs`) are unchanged; re-verify file:line citations if Phase 16 or a hotfix touches any of the cited files before Phase 15 executes.
