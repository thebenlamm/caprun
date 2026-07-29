# Phase 49: Deterministic Multi-step Coding Planner - Pattern Map

**Mapped:** 2026-07-29
**Files analyzed:** 9
**Analogs found:** 9 / 9

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/runtime-core/src/intent.rs` | model | transform | same file (`CaprunIntent` email/file variants) | exact |
| `crates/brokerd/src/proto.rs` | route / wire | request-response | same file (`IntentAccepted` Phase-15 additive slots) | exact |
| `crates/brokerd/src/server.rs` | controller | request-response | same file ProvideIntent arm (`mint_from_intent` chain) | exact |
| `cli/caprun/src/planner.rs` | service | transform | `DeterministicPlanner` + default `plan_next` + `MultiNodeTestPlanner` | exact + role-match |
| `cli/caprun/src/worker.rs` | controller | request-response + event-driven | same file bag seed + claim-extract match | exact |
| `cli/caprun/src/main.rs` | config / CLI | request-response | same file `intent_kind` match (fail-closed unknown) | exact |
| `cli/caprun/tests/planner.rs` | test | transform | `MultiNodeTestPlanner` + email/file `plan_next` tests | exact |
| `crates/brokerd/tests/proto_claims.rs` | test | request-response | `provide_intent_dispatch_returns_intent_accepted…` | exact |
| `crates/executor/src/sink_schema.rs` | config (read-only) | transform | same file sink rows for write/exec/commit/push/pr | exact (no edit expected) |

## Pattern Assignments

### `crates/runtime-core/src/intent.rs` (model, transform)

**Analog:** same file — closed `CaprunIntent` enum with `#[serde(tag = "kind")]`

**Imports / derive pattern** (lines 20–22):
```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum CaprunIntent {
```

**Doc + multi-field trusted mint contract** (lines 23–32 — copy discipline for coding variant docs):
```rust
/// `recipient`/`subject`/`body` are user-provided literals, each minted as
/// its OWN DISTINCT `UserTrusted` `ValueRecord` by three sequential
/// `mint_from_intent` calls in the broker's `ProvideIntent` arm (Phase 15
/// finding #6) — the planner receives only the three opaque `ValueId`
/// handles, never the literals directly.
```

**Existing variants** (lines 33–47) — keep byte-stable; **ADD** closed coding variant (research recommends `SafeCodingWorkflow` with ~12 operator string fields: path, contents, test_command, test_args_json, commit_message, remote, refspec, owner, repo, base, head, pr_title, pr_body). No free-form map; no new crates.

**Pattern to copy:**
- Closed enum only; each field is an operator-typed `String` literal for ProvideIntent mint.
- Comment that planner must ignore struct fields via `..` (PLAN-03) — literals live in broker ValueStore only.
- Gate 2: pure types only (no I/O/async) — already satisfied by this crate.

---

### `crates/brokerd/src/proto.rs` (route / wire, request-response)

**Analog:** same file — Phase 15 additive `subject_value_id` / `body_value_id` on `IntentAccepted`

**ProvideIntent wire** (lines 109–112) — unchanged shape; coding intent rides `intent: CaprunIntent`:
```rust
ProvideIntent {
    intent: runtime_core::intent::CaprunIntent,
    primary_file_derived: bool,
},
```

**IntentAccepted additive extension pattern** (lines 207–223):
```rust
/// `subject_value_id`/`body_value_id` are ADDITIVE (Phase 15, 15-04,
/// finding #6): for a `SendEmailSummary` intent the broker mints THREE
/// DISTINCT `UserTrusted` handles ...
IntentAccepted {
    value_id: runtime_core::plan_node::ValueId,
    subject_value_id: Option<runtime_core::plan_node::ValueId>,
    body_value_id: Option<runtime_core::plan_node::ValueId>,
},
```

**Pattern to copy for Phase 49:**
- Add **one additive** field, e.g. `named_handles: Vec<(String, ValueId)>` (or `BTreeMap` if stable serde preferred).
- Prefer **explicit empty vec at every construction site** over silent `#[serde(default)]` for security-relevant provenance (mirror `primary_file_derived` / `output_value_id` discipline at lines 105–108 of same file).
- Email/file keep three-slot semantics: set `named_handles` to empty (or only coding fills it); coding may put primary under `value_id` (e.g. write_path) and the rest in the map.
- Exhaustive construction update: every `BrokerResponse::IntentAccepted { … }` site in broker + tests must name the new field.

---

### `crates/brokerd/src/server.rs` (controller, request-response)

**Analog:** ProvideIntent arm — once-before-RequestFd guard + sequential `mint_from_intent` (lines 2372–2557)

**Once guard** (lines 2382–2393) — **do not change**:
```rust
if *intent_provided || *fd_requested {
    send_response(
        stream,
        &BrokerResponse::Error {
            message: "ProvideIntent rejected: must arrive exactly once, \
                      before any RequestFd (fail-closed)"
                .into(),
        },
    )
    .await?;
    return Ok(());
}
```

**Exhaustive intent match extracting literals + roles** (lines 2417–2434):
```rust
let (primary_literal, primary_role, primary_claim_type, subject_literal, body_literal): (
    String,
    &'static str,
    &'static str,
    Option<String>,
    Option<String>,
) = match &intent {
    CaprunIntent::SendEmailSummary { recipient, subject, body } => (
        recipient.clone(),
        "recipient",
        "email_address",
        Some(subject.clone()),
        Some(body.clone()),
    ),
    CaprunIntent::CreateFileFromReport { path } => {
        (path.clone(), "path", "relative_path", None, None)
    }
};
```

**Sequential mint chain with chain-head threading** (lines 2487–2538):
```rust
let (event_id, hash, value_id) = mint_from_intent(
    &locked,
    key,
    value_store,
    session_id,
    primary_literal,
    Some(*last_event_id),
    Some(last_event_hash),
    Some(primary_role.to_string()),
)?;
// then subject/body: each mint advances *last_event_id / *last_event_hash
```

**Response construction** (lines 2548–2555):
```rust
send_response(
    stream,
    &BrokerResponse::IntentAccepted {
        value_id,
        subject_value_id,
        body_value_id,
    },
)
.await?;
```

**Pattern to copy for coding multi-mint:**
1. Extend the extract match with `CaprunIntent::SafeCodingWorkflow { … }` (or chosen name).
2. Prefer a **loop over named (key, literal, role)** pairs rather than exploding the 5-tuple — still only call `mint_from_intent` inside this arm (Gate 3).
3. Role tags: write `path` / `contents` use `Some("path")` (LIVE-05 / Step 1c); other coding fields may use descriptive roles or `None` per `sink_sensitivity`.
4. `primary_file_derived` for coding success path must be **false** (operator-typed); do not route coding args through `mint_from_read`.
5. Return distinct ValueIds per literal (Phase 15 finding #6 — never reuse one handle for path/command/PR body).
6. Mark `*intent_provided = true` only after all mints succeed (existing pattern lines 2543–2546).
7. Populate additive `named_handles` on `IntentAccepted`; email/file pass empty vec.

**Hybrid mint role reference (LIVE, not product path):** `cli/caprun/tests/live_acceptance_v1_9_composed.rs` helper ~210–236 — `mint_from_intent(…, origin_role: Some("path"))` for file.write path/contents.

---

### `cli/caprun/src/planner.rs` (service, transform)

**Analog A:** default `plan_next` one-shot adapter (lines 134–161) — keep for email/file  
**Analog B:** `plan_from_intent` PLAN-03 handles-only (lines 236–294)  
**Analog C:** `cli/caprun/tests/planner.rs` `MultiNodeTestPlanner::plan_next` (lines 918–960) — static step-index sequence

**PlanStreamContext bag contract** (lines 75–99):
```rust
/// | Key | Meaning |
/// | `intent` | UserTrusted primary intent handle |
/// | `trusted_subject` / `trusted_body` | UserTrusted email slots |
/// | `out_{step}` | Allowed-path output_value_id (F-01) |
pub struct PlanStreamContext {
    pub intent: CaprunIntent,
    pub step_index: usize,
    pub handles: HashMap<String, ValueId>,
    pub task_instruction: Option<String>,
}
```

**Default one-shot adapter** (lines 143–161) — email/file must stay green:
```rust
fn plan_next(&self, ctx: &PlanStreamContext) -> Option<PlanNode> {
    if ctx.step_index != 0 {
        return None;
    }
    let intent_value_id = ctx.handles.get("intent")?.clone();
    let trusted_subject_handle = ctx.handles.get("trusted_subject")?.clone();
    let trusted_body_handle = ctx.handles.get("trusted_body")?.clone();
    // …
    Some(self.plan(/* … */))
}
```

**PLAN-03 plan_from_intent shape** (lines 244–267) — place handles, ignore intent literals:
```rust
match intent {
    CaprunIntent::SendEmailSummary { .. } => {
        PlanNode {
            sink: SinkId("email.send".into()),
            args: vec![
                PlanArg { name: "to".into(), value_id: to },
                PlanArg { name: "subject".into(), value_id: trusted_subject_handle },
                PlanArg { name: "body".into(), value_id: body_value_id },
            ],
        }
    }
    CaprunIntent::CreateFileFromReport { .. } => { /* file.create */ }
}
```

**MultiNodeTestPlanner static step pattern** (`cli/caprun/tests/planner.rs` 918–960):
```rust
fn plan_next(&self, ctx: &planner::PlanStreamContext) -> Option<PlanNode> {
    match ctx.step_index {
        0 => { /* build PlanNode from ctx.handles.get("…")? */ }
        1 => { /* bag handle placement */ }
        _ => None,
    }
}
```

**Pattern to copy for coding arm (recommend override on `DeterministicPlanner::plan_next`):**
| step | sink | PlanArg names (exact from sink_schema) | bag keys |
|------|------|----------------------------------------|----------|
| 0 | `file.write` | `path`, `contents` | `write_path`, `write_contents` |
| 1 | `process.exec` | `command`, optional `args` | `test_command`, `test_args` |
| 2 | `git.commit` | `message` | `commit_message` |
| 3 | `git.push` | `remote`, `refspec` | `push_remote`, `push_refspec` |
| 4 | `github.pr` | `owner`, `repo`, `base`, `head`, `title`, `body` | `pr_*` |

- Missing bag key → `None` (fail-closed), never invent literals.
- Success path: **never** `handles.get("out_*")` into sink args.
- Non-coding intents: fall through to default one-shot adapter (or call `Planner::plan_next` super-pattern).
- `plan_from_intent`: add coding arm that either unreachable / fail-closed (coding uses `plan_next` only) or single-node stub — do not invent multi-node in `plan()`.
- `LlmPlanner` / `build_planner_request` match arms (lines 518–524, 531+): coding arm **fail-closed** (exit 1 / no multi-step LLM).

---

### `cli/caprun/src/worker.rs` (controller, request-response + stream)

**Analog:** ProvideIntent receive + claim-extract match + bag seed (lines 170–382)

**IntentAccepted destructure** (lines 184–194) — must extend for `named_handles`:
```rust
let (intent_value_id, subject_value_id, body_value_id) =
    match recv_framed::<BrokerResponse>(&std_stream)? {
        BrokerResponse::IntentAccepted {
            value_id,
            subject_value_id,
            body_value_id,
        } => (value_id, subject_value_id, body_value_id),
        other => anyhow::bail!("unexpected response to ProvideIntent: {other:?}"),
    };
```

**Claim-extract exhaustive match** (lines 227–332) — email/file arms stay; **add coding arm**:
```rust
match &intent {
    CaprunIntent::SendEmailSummary { .. } => { /* ReportClaims + derived */ }
    CaprunIntent::CreateFileFromReport { .. } => { /* RelativePath claims */ }
    // NEW: SafeCodingWorkflow — skip claim extract / no multi-file demotion path
}
```

**Bag seed** (lines 373–382):
```rust
let mut bag: HashMap<String, ValueId> = HashMap::new();
bag.insert("intent".into(), intent_value_id);
// …
bag.insert("trusted_subject".into(), trusted_subject_handle);
bag.insert("trusted_body".into(), trusted_body_handle);
```

**Stream loop already shipped** (lines 384–430) — **do not re-design**:
- `plan_next` → `SubmitPlanNode` → branch Allowed/Block/Deny
- `out_{step}` insert on Allowed + Some(output_value_id)

**Pattern to copy:**
1. Destructure `named_handles` from `IntentAccepted`.
2. Coding match arm: seed bag keys from named handles (`write_path`, …); **no** claim-driven demotion for success path; prefer skip multi-file untrusted RequestFd / claim extract (research Open Q2 — document choice).
3. Keep email/file RequestFd + claim path unchanged.
4. Do not re-ProvideIntent mid-stream (already guaranteed by loop design).

---

### `cli/caprun/src/main.rs` (CLI / config, request-response)

**Analog:** intent-kind string map ~lines 309–315

```rust
let intent = match intent_kind.as_str() {
    "send-email-summary" => CaprunIntent::SendEmailSummary { … },
    "create-file-from-report" => CaprunIntent::CreateFileFromReport { … },
    // unknown → fail-closed
};
```

**Pattern:** Phase 49 may leave coding kind fail-closed (product verb = Phase 50). If a test-only parse path is added, keep it non-product. Exhaustive compile will force a touch if any other match on CaprunIntent lives here — prefer `_ =>` error still covers new variant if map is string-based only.

---

### `cli/caprun/tests/planner.rs` (test, transform)

**Analog:** `MultiNodeTestPlanner` + existing plan_next email/file regression tests

**Copy for CODE-01:**
- Construct `CaprunIntent::SafeCodingWorkflow { … }` in-process.
- Seed bag with **intent-minted key set only** (simulate named_handles).
- Assert `plan_next` at steps 0..4 emits exact sinks + arg names; step ≥5 → None.
- Assert email/file `plan_next` step0 still matches `plan()`; step≥1 → None.

**Copy for CODE-02 anti-launder:**
```rust
// For each success-path node arg value_id:
// assert it is NOT equal to any bag entry whose key starts with "out_"
```

**Copy for LIVE-08 expressibility (test-only planner, not success path):**
- Second test planner places `out_1` into e.g. `github.pr`/`body` — proves bag can route tainted handles without changing success recipe.

---

### `crates/brokerd/tests/proto_claims.rs` (test, request-response)

**Analog:** `intent_accepted_response_round_trips` (lines 81–95) + `provide_intent_dispatch_returns_intent_accepted_with_resolvable_handle` (lines 104–221)

**Round-trip pattern:**
```rust
let resp = BrokerResponse::IntentAccepted {
    value_id: value_id.clone(),
    subject_value_id: Some(ValueId::new()),
    body_value_id: Some(ValueId::new()),
    // NEW: named_handles: vec![…] or empty
};
```

**Dispatch + distinct handles pattern** (lines 178–187):
```rust
assert_ne!(subject_value_id, value_id, "… DISTINCT …");
assert_ne!(body_value_id, value_id, "…");
// resolve in store; assert UserTrusted; no untrusted labels
```

**Pattern for coding mint test:**
- ProvideIntent with coding variant + `primary_file_derived: false`.
- Assert N distinct named handles; each resolves; literals match intent fields; roles OK for path/contents.
- Update **all** existing `IntentAccepted { … }` destructures/constructors in this file for the new field.

---

### `crates/executor/src/sink_schema.rs` (config, read-only reference)

**Analog / source of truth** — Phase 49 should **not** invent arg names; copy exact sets:

| sink | allowed / required (lines) |
|------|----------------------------|
| `file.write` | `path`, `contents` (62–64) |
| `process.exec` | required `command`; optional `args`, `cwd` (72–74) — `args` literal is JSON `Vec<String>` at process_exec sink |
| `git.commit` | `message` only (88–90) — already-staged only |
| `git.push` | `remote`, `refspec` (157–159) |
| `github.pr` | `owner`, `repo`, `base`, `head`, `title`, `body` (117–119) |

No schema edits expected for CODE-01/02 unless a name mismatch is found (then fix planner, not schema).

---

## Shared Patterns

### ProvideIntent-once + Gate-3 mint
**Source:** `crates/brokerd/src/server.rs` lines 2372–2557  
**Apply to:** coding multi-mint only inside this arm via `mint_from_intent`  
- No mid-stream ProvideIntent  
- No planner/worker `.mint`  
- Linear chain-head threading across sequential mints  

### PLAN-03 handles-only planner
**Source:** `cli/caprun/src/planner.rs` `plan_from_intent` + trait docs  
**Apply to:** coding `plan_next`  
- Match `CaprunIntent::… { .. }` — never read string fields for PlanArg  
- Only place `ValueId`s from bag  
- `task_instruction` is String framing only — never bindable  

### Opaque handle bag
**Source:** `cli/caprun/src/worker.rs` lines 367–428 + `PlanStreamContext`  
**Apply to:** coding bag seed + stream  
- Keys → ValueId only  
- Seed once from ProvideIntent named handles  
- Store `out_{step}` on Allowed; success recipe must not consume those keys  

### Additive wire discipline (no silent defaults)
**Source:** `crates/brokerd/src/proto.rs` ProvideIntent `primary_file_derived` docs (105–108); Phase 15 IntentAccepted additive fields  
**Apply to:** `named_handles` field  
- Explicit construction at every site  
- Email/file: empty named map  

### Fail-closed exhaustive CaprunIntent matches
**Apply to:** server ProvideIntent, planner `plan_from_intent` / LlmPlanner / `build_planner_request`, worker claim extract, tests  
- Compiler forces arm updates when variant is added  
- LlmPlanner coding: exit 1 / refuse, no tool-use multi-step  

### Email/file regression (CODE-01 criterion 3)
**Source:** default `plan_next` adapter + existing planner tests  
**Apply to:** any DeterministicPlanner change  
- Keep one-shot adapter for non-coding  
- Do not force email/file through five-node recipe  

### HYG-02 / Gate 1
**Source:** `scripts/check-invariants.sh`  
**Apply to:** all crates  
- Zero new crates  
- No `EffectRequest` token under `crates/`  
- Gate 3: mint sites remain quarantine + server  

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| *(none for core CODE-01/02 path)* | — | — | Coding recipe is new product logic but every touchpoint has a same-file or Phase-48 analog |

**Discretion / partial-only (not “no analog”):**
- Exact `named_handles` serde type (`Vec<(String,ValueId)>` vs map) — no prior N-named map; Phase-15 Option slots are the additive precedent.
- Skipping RequestFd for coding success path — worker always RequestFd today; closest pattern is “branch by intent kind” in claim extract, not a prior skip.

## Exhaustive Match Blast Radius (compile sweep)

When adding `CaprunIntent` variant, update at least:

| Locus | Pattern |
|-------|---------|
| `crates/brokerd/src/server.rs` ProvideIntent | multi-mint arm |
| `cli/caprun/src/planner.rs` `plan_from_intent` | fail-closed or unused |
| `cli/caprun/src/planner.rs` `build_planner_request` / LlmPlanner | fail-closed |
| `cli/caprun/src/worker.rs` claim match | skip claims / seed bag |
| `cli/caprun/src/main.rs` | optional kind string (Phase 50 product) |
| All `IntentAccepted { … }` construction/destructure | additive field |
| Tests constructing intents / IntentAccepted | proto_claims, stream_*, planner, etc. |

## Metadata

**Analog search scope:** `crates/runtime-core`, `crates/brokerd` (src + tests), `cli/caprun` (src + tests), `crates/executor/src/sink_schema.rs`  
**Files scanned:** ~15 primary + grep over brokerd/tests IntentAccepted sites  
**Pattern extraction date:** 2026-07-29  
**Phase boundary reminder for planner:** Phase 49 = recipe + mint/bag seed + unit tests. **Not** CLI multi-node driver (50), Block-and-Hold (50), LIVE-07/08 (51).
