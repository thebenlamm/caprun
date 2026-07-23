# Phase 24: Slot-Type Binding Enforcement — Pattern Map

**Mapped:** 2026-07-12
**Files analyzed:** 7 production files modified (no new files — all edits to existing TCB files, per DESIGN §0/§9 scope)
**Analogs found:** 7 / 7 (all self-analogous — every touched file's own existing sibling pattern in the SAME file is the analog; this phase extends established taxonomies rather than introducing new file roles)

DESIGN wins on all decisions below. No RESEARCH/DESIGN conflicts found; RESEARCH's own file:line citations were spot-verified against live code during this mapping (all confirmed byte-identical) and are cited directly rather than re-quoted.

## File Classification

| File to modify | Role | Data Flow | Closest Analog (same file, existing pattern) | Match Quality |
|---|---|---|---|---|
| `crates/runtime-core/src/value_record.rs` | model (struct field) | CRUD (additive field) | own `taint`/`provenance_chain` fields | exact |
| `crates/executor/src/value_store.rs` (`ValueStore::mint`) | service (sole constructor) | CRUD (create) | own existing 3-param signature | exact |
| `crates/brokerd/src/quarantine.rs` (3 `mint_from_*` fns) | service (wrapper/orchestration) | CRUD (create, additive param) | own existing wrapper bodies | exact |
| `crates/brokerd/src/server.rs` (5 mint dispatch call sites) | controller (request dispatch) | request-response | own existing intent-variant match + call sites | exact |
| `crates/executor/src/sink_sensitivity.rs` (new `expected_role`) | utility (hardcoded lookup table) | transform (pure fn) | `is_routing_sensitive`/`is_content_sensitive` in the SAME file | exact — DESIGN §3 explicitly mandates mirroring this |
| `crates/runtime-core/src/executor_decision.rs` (`DenyReason::SlotTypeMismatch` + 2 matches) | model (typed enum + exhaustive matches) | transform (classify) | own existing `DraftOnlySessionDeniesCommitIrreversible`/`NonLiveSessionDeniesCommitIrreversible` variants | exact |
| `crates/executor/src/lib.rs` (`submit_plan_node`, Step 1c) | controller (enforcement gate) | request-response | own existing Steps 1/1a/1b (structural per-arg guards) | exact |
| Test files (~63 call sites, 8 files) | test | CRUD (fixture construction) | own existing fixture calls, mechanical arg addition | exact |

## Pattern Assignments

### `crates/runtime-core/src/value_record.rs` (model)

**Analog:** the struct's own existing 4 fields (`value_record.rs:20-31`)

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ValueRecord {
    pub id: ValueId,
    pub literal: String,
    pub taint: Vec<TaintLabel>,
    pub provenance_chain: Vec<uuid::Uuid>,
}
```

**Pattern to apply (DESIGN §1, F6):** add a 5th field, `#[serde(default)]` because there is NO `Default` derive on this struct — every direct struct-literal site must be updated or the build fails (fail-closed by construction, not accidental):
```rust
#[serde(default)]
pub origin_role: Option<String>,
```
No error handling / validation pattern needed here — it's a plain field, `Option<String>` per DESIGN §1 ("not bare `String`... `None` must be representable").

**3 direct struct-literal sites to update (all test-only, confirmed by RESEARCH Existing-Code Map §1):** `crates/executor/src/value_store.rs:74-79` (the ONE production constructor — see next section), `crates/brokerd/src/quarantine.rs:1589-1600` (2 sites, `#[cfg(test)]`), `crates/runtime-core/tests/types_compile.rs:42-47`.

---

### `crates/executor/src/value_store.rs` — `ValueStore::mint` (service, sole constructor)

**Analog:** its own current signature (`value_store.rs:61-82`, verified live):
```rust
pub fn mint(
    &mut self,
    literal: String,
    taint: Vec<TaintLabel>,
    provenance_chain: Vec<uuid::Uuid>,
) -> Result<ValueId, MintInvariantError> {
    if taint.is_empty() {
        return Err(MintInvariantError::EmptyTaint);
    }
    if provenance_chain.is_empty() {
        return Err(MintInvariantError::EmptyProvenance);
    }
    let id = ValueId::new();
    let record = ValueRecord {
        id: id.clone(),
        literal,
        taint,
        provenance_chain,
    };
    self.inner.insert(id.clone(), record);
    Ok(id)
}
```

**Pattern to apply (RESEARCH A4, Gate-3-legal):** add a 4th parameter `origin_role: Option<String>`, threaded straight into the `ValueRecord { .. }` literal — do NOT add validation logic for it (unlike `taint`/`provenance_chain`, there is no fail-closed guard on `origin_role` at mint time; DESIGN's fail-closed check lives entirely in `submit_plan_node` Step 1c, not here). Error-handling pattern (early-return `Result` on empty taint/provenance) is UNCHANGED — origin_role is not subject to an empty-check.

**Existing test fixture sites in this file's own `#[cfg(test)] mod tests`:** `value_store.rs:111,146,158,179` (4 sites) — mechanical `None`/`Some("...")` addition.

---

### `crates/brokerd/src/quarantine.rs` — 3 `mint_from_*` wrappers (service, orchestration)

**Analog:** each wrapper's own existing signature + its `store.mint(...)` delegation line (RESEARCH Existing-Code Map §2, confirmed):
- `mint_from_read` (`:284-297`, delegates at `:365`) — role source: `claim.claim_type` verbatim, already resolved inside the fn body (`:315-341`) — reuse it, don't re-derive.
- `mint_from_intent` (`:435-442`, delegates at `:470`) — role source: NEW caller-supplied parameter (the function has no internal way to know its own semantic field — caller in `server.rs` decides).
- `mint_from_derivation` (`:574-583`, delegates at `:703`) — role source: hardcoded inside the fn's OWN existing `match transform_kind { "concat" => ... }` block (`:670-692`), mirroring how taint is already computed in that same match — **but role must NOT read `inputs[*]` the way taint does** (DESIGN §4 anti-laundering; Pitfall 3).

**Core pattern to copy** — the `"concat"` arm's existing byte-verify guard shape (add the arity guard alongside it, same `if`/`else` idiom already used for the byte-verify check at `:671-685`):
```rust
// existing byte-verify guard shape at quarantine.rs:671-685 — add role assignment
// in the SAME arm, following the SAME explicit-branch idiom (RESEARCH §9, F2):
let origin_role = if inputs.len() == 2 {
    Some("recipient".to_string())
} else {
    None
};
```

**Error handling:** unchanged — existing fail-closed `Result` returns for zero-input concat (`:588-593`) and unknown `claim_type` (`:336-340`) are untouched; `origin_role` threading adds no new error path.

**Test fixture blast radius (mechanical, this file's own `#[cfg(test)] mod tests`):** ~44 call sites — the single largest concentration (RESEARCH Existing-Code Map §3).

---

### `crates/brokerd/src/server.rs` — 5 dispatch call sites (controller)

**Analog:** the file's own existing intent-variant match (`server.rs:1294-1300`, confirmed live) — this IS the F3 trap RESEARCH flags:
```rust
let (primary_literal, subject_literal, body_literal) = match &intent {
    CaprunIntent::SendEmailSummary { recipient, subject, body } =>
        (recipient.clone(), Some(subject.clone()), Some(body.clone())),
    CaprunIntent::CreateFileFromReport { path } => (path.clone(), None, None),
};
```

**Pattern to apply (DESIGN §2 Round-1 F3 — MUST follow exactly):** extend the SAME match to also produce `primary_role: &'static str` in the same arm as `primary_literal`:
```rust
let (primary_literal, primary_role, subject_literal, body_literal) = match &intent {
    CaprunIntent::SendEmailSummary { recipient, subject, body } =>
        (recipient.clone(), "recipient", Some(subject.clone()), Some(body.clone())),
    CaprunIntent::CreateFileFromReport { path } => (path.clone(), "path", None, None),
};
```
Then thread `primary_role` into the shared `mint_from_intent` call at `:1317-1324`. The `subject`/`body` call sites (`:1330-1337`, `:1347-1354`) get literal `"subject"`/`"body"` — no ambiguity, no match needed (single intent variant reaches each).

**Anti-pattern (explicit warning, both DESIGN and RESEARCH flag this identically):** never hardcode `"recipient"` at the shared `:1317` call site itself — it is reached by BOTH intent variants.

`ReportClaims` (`:1084-1094`) and `ReportDerivedClaim` (`:1217-1228`) call sites need NO role parameter added at the call site — role is resolved inside the callee (`claim.claim_type` / the `"concat"` match) per above.

---

### `crates/executor/src/sink_sensitivity.rs` (utility, hardcoded lookup)

**Analog — the exact structure to mirror, verified live** (`sink_sensitivity.rs:61-93`):
```rust
pub const EMAIL_SEND_ROUTING_SENSITIVE: &[&str] = &["to", "cc", "bcc"];
pub const FILE_CREATE_ROUTING_SENSITIVE: &[&str] = &["path"];
pub const EMAIL_SEND_CONTENT_SENSITIVE: &[&str] = &["subject", "body"];

pub fn is_routing_sensitive(sink: &SinkId, arg_name: &str) -> bool {
    match sink.0.as_str() {
        "email.send" => EMAIL_SEND_ROUTING_SENSITIVE.contains(&arg_name),
        "file.create" => FILE_CREATE_ROUTING_SENSITIVE.contains(&arg_name),
        _ => false,
    }
}
```
Doc-comment convention on these consts: security-property framing ("A tainted value in any of these args → Block"), not a generic comment — new code should carry the same framing (CON-i2-non-bypassable).

**New fn to add, mirroring the shape exactly (DESIGN §3, pinned table — RESEARCH already drafted the body, confirmed correct against DESIGN):**
```rust
pub fn expected_role(sink: &SinkId, arg_name: &str) -> Option<&'static [&'static str]> {
    match sink.0.as_str() {
        "email.send" => match arg_name {
            "to" | "cc" | "bcc" => Some(&["recipient", "email_address"]),
            "subject" => Some(&["subject"]),
            "body" => Some(&["body"]),
            _ => None,
        },
        "file.create" => match arg_name {
            "path" => Some(&["path", "relative_path"]),
            "contents" => None, // unconstrained for v1.5 — Assumption A2
            _ => None,
        },
        _ => None, // any other sink: unconstrained, out of v1.5 scope
    }
}
```
**Contract discipline (fail-closed, DESIGN §7):** `None` = unconstrained/no-op; `Some(&[])` must never be constructed; the CALLER (Step 1c below) must never write `.unwrap_or(&[])` — that's the #1 flagged anti-pattern in both DESIGN §8 angle 2 and RESEARCH Pitfall 4.

**Test analog:** `#[cfg(test)] mod tests` already exists at the bottom of this file (`sink_sensitivity.rs`, verified present) — add `expected_role` cases in the SAME module, same assertion style as existing `is_routing_sensitive`/`is_content_sensitive` tests.

---

### `crates/runtime-core/src/executor_decision.rs` — `DenyReason` (model, exhaustive enum)

**Analog — the exact variant + doc-comment convention to mirror** (`executor_decision.rs:14-60`, verified live):
```rust
/// A `CommitIrreversible` plan node was submitted while the session is
/// `SessionStatus::Draft` and no per-arg I2 Block already fired. Carries the
/// offending `SinkId`...
DraftOnlySessionDeniesCommitIrreversible { sink: crate::plan_node::SinkId },
```

**New variant to add (DESIGN §5, F1-corrected owned types — load-bearing, `DenyReason` derives `Deserialize` and crosses IPC):**
```rust
/// A resolved value's origin-role tag did not match its slot's expected-role
/// set (T2, DESIGN-slot-type-binding.md §5/§7). Structural fail-closed —
/// never confirmable, never BlockedPendingConfirmation.
SlotTypeMismatch { sink: String, arg: String, expected: Vec<String>, found: Option<String> },
```
**Note the type deviation from `DraftOnly...`'s pattern:** existing variants carry `SinkId` (a typed wrapper); `SlotTypeMismatch` MUST use plain owned `String`/`Vec<String>`/`Option<String>` — NOT `&'static [&'static str]` — because of the serde-IPC constraint (DESIGN §5 Round-1 F1). Don't copy the `SinkId`-typed convention here.

**The two exhaustive matches to extend, exact current shape verified live:**
```rust
// code() — executor_decision.rs:64-80
DenyReason::NonLiveSessionDeniesCommitIrreversible { .. } => {
    "non_live_session_denies_commit_irreversible"
}
// ADD: DenyReason::SlotTypeMismatch { .. } => "slot_type_mismatch",

// Display — executor_decision.rs:83-112
DenyReason::NonLiveSessionDeniesCommitIrreversible { sink } => write!(
    f, "non-live session (...) denies CommitIrreversible sink `{sink}`", sink = sink.0
),
// ADD: DenyReason::SlotTypeMismatch { sink, arg, expected, found } => write!(
//     f, "value routed into `{arg}` of sink `{sink}` has role {found:?}, expected one of {expected:?}"
// ),
```
No wildcard arm in either match — confirmed both are still fully exhaustive with named variants only, matching `check-invariants.sh`'s no-wildcard discipline.

**Test analog note (RESEARCH flags this as a gap, not an existing pattern):** this file currently has NO `#[cfg(test)] mod tests` — verify before assuming one exists; if absent, follow the sibling-crate convention (`sink_sensitivity.rs`'s bottom-of-file `#[cfg(test)]` module) rather than inventing a new test-file location.

---

### `crates/executor/src/lib.rs` — `submit_plan_node`, Step 1c (controller, enforcement)

**Analog — Steps 1/1a/1b, the exact structural-guard idiom to copy** (`lib.rs:81-109`, verified live):
```rust
// Step 1b: Empty-provenance guard...
if record.provenance_chain.is_empty() {
    return ExecutorDecision::Denied {
        reason: DenyReason::MissingProvenanceAnchor,
    };
}
```

**Step 1c to insert here, immediately after this block, BEFORE the existing `let sensitive = ...` line (`:117`):**
```rust
// Step 1c: role check (NEW, T2). Structural per-arg guard — fires BEFORE
// this arg is considered for sensitivity collection, same tier as 1/1a/1b.
if let Some(expected) = sink_sensitivity::expected_role(&plan_node.sink, &arg.name) {
    let role_ok = record
        .origin_role
        .as_deref()
        .is_some_and(|r| expected.contains(&r));
    if !role_ok {
        return ExecutorDecision::Denied {
            reason: DenyReason::SlotTypeMismatch {
                sink: plan_node.sink.0.clone(),
                arg: arg.name.clone(),
                expected: expected.iter().map(|s| s.to_string()).collect(),
                found: record.origin_role.clone(),
            },
        };
    }
}
// None (unconstrained) or matching role: fall through unchanged.
```
**Precedence pattern to preserve, copied verbatim from the existing load-bearing comment at `:169-172`:** "the per-arg I2 Block always takes precedence over this I1/I0 class-level deny" — Step 1c must return per-arg, BEFORE the `blocked.push(...)` collection step, exactly like 1/1a/1b do, never joining the `blocked: Vec<BlockedArg>` collect-then-Block pattern used by Steps 2/3.

**Anti-pattern (explicit, both DESIGN §6 and RESEARCH Pitfall 4/anti-patterns list):** do NOT fold this into the `blocked.push` loop; do NOT use `.unwrap_or(&[])` when reading `expected_role`'s return.

---

### Test files (~63 mechanical call sites, 8 files)

**Concentration sites (verification checkpoints, per RESEARCH):**
| File | Count | Pattern |
|---|---|---|
| `crates/brokerd/src/quarantine.rs` `#[cfg(test)] mod tests` | ~44 | add `origin_role` arg to existing `mint_from_*` calls |
| `crates/executor/tests/executor_decision.rs` | 16 `.mint()` calls | add 4th arg to `store.mint(...)` calls; ALSO add 3 new test cases per DESIGN §7 (mismatch→Denied, matching-role+tainted→still Blocks, unconstrained-slot→unaffected) |
| `crates/executor/src/value_store.rs` own tests | 4 | mechanical |
| `crates/brokerd/tests/{extract_provenance_threading,s9_acceptance,durable_anchor,phase5_dispatch}.rs`, `cli/caprun/tests/live_acceptance_v1_3.rs` | 5+3+1+4+1 | mechanical |

**Pattern:** explicit `origin_role: None` or `Some("...".to_string())` at each site — DESIGN's "Don't Hand-Roll" table explicitly forbids a blanket `#[derive(Default)]` shortcut here (would mask which fixtures intentionally exercise a role vs don't).

---

## Shared Patterns

### Hardcoded TCB lookup table discipline
**Source:** `crates/executor/src/sink_sensitivity.rs:86-93` (`is_routing_sensitive`)
**Apply to:** `expected_role()` — same file, same `match sink.0.as_str() { "email.send" => ..., "file.create" => ..., _ => ... }` shape, doc-commented as a security property (CON-i2-non-bypassable), never a config file or pluggable framework.

### Exhaustive no-wildcard enum-match discipline
**Source:** `crates/runtime-core/src/executor_decision.rs:64-112` (both `code()` and `Display`); `crates/executor/src/lib.rs:176` (`SessionStatus` match, untouched by this phase but same discipline)
**Apply to:** the 2 `DenyReason` matches gaining `SlotTypeMismatch`. `check-invariants.sh`'s no-wildcard gate is the compile-time backstop — a missed 3rd match site fails the build rather than silently mis-rendering.

### Fail-closed `Option`-not-bare-collection contract
**Source:** DESIGN §7, echoed by `sink_sensitivity.rs`'s existing `bool`-returning sibling functions (which do NOT face this ambiguity because `bool` has no third state)
**Apply to:** `expected_role`'s `Option<&'static [&'static str]>` return AND its call site in Step 1c — never collapse via `.unwrap_or(&[])`.

### Structural per-arg guard tier (return-immediately, not collect-then-Block)
**Source:** `crates/executor/src/lib.rs:81-109` (Steps 1/1a/1b)
**Apply to:** Step 1c — same tier, same immediate-return idiom, explicitly NOT the `blocked.push()` collect-then-Block pattern used one tier down (Steps 2/3).

## No Analog Found

None — every file in scope is an incremental extension of an existing, already-analogous pattern within the SAME file (this phase adds zero new files and zero new architectural roles, per DESIGN §0's tight scope).

## Metadata

**Analog search scope:** `crates/runtime-core/src/{value_record,executor_decision,plan_node}.rs`, `crates/executor/src/{lib,value_store,sink_sensitivity}.rs`, `crates/brokerd/src/{quarantine,server}.rs`, plus the 8 test files RESEARCH enumerated.
**Files scanned:** 7 production files (live-read, byte-verified against RESEARCH's citations) + 8 test files (counted via RESEARCH's fresh greps, not independently re-read — mechanical, no new pattern).
**Pattern extraction date:** 2026-07-12

## PATTERN MAPPING COMPLETE
