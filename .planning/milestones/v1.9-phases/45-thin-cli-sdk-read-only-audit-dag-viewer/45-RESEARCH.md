# Phase 45: Thin CLI/SDK + Read-Only Audit-DAG Viewer - Research

**Researched:** 2026-07-18
**Domain:** caprun CLI surface (cli/caprun binary) + read-only SQLite audit-DAG rendering; MAC-key custody reuse; tainted-byte display neutralization
**Confidence:** HIGH (all anchors re-verified against HEAD `89cab31`, code read directly)

## Summary

Phase 45 is a **thin binary-surface + one shared-helper extraction** phase, NOT deep TCB
executor work. Almost everything SDK-01 and U1 need already exists and is shipping: the
`caprun` binary already dispatches `confirm`/`deny`/`review`/`grant` (main.rs:75-146), the run
path already binds the trusted policy at session creation from `CAPRUN_POLICY` via
`bind_policy` (main.rs:289-294, POLICY-03 enforcement point), `load_or_create_key` already
reuses the extracted F1 containment helper (key.rs:78), `verify_chain(conn, session_id, key)`
is already a pure read-only function (audit.rs:1240), `query_events_by_session` reads the DAG
(audit.rs:1056), and `neutralize_control_chars` already exists (confirmation.rs:575). The work
is to **surface, reuse, and generalize** these — not build new TCB.

Four genuine wiring gaps drive the plan (details in Wiring Gaps): (1) **the M7 laundering
gap** — `--seed-from-file` mints file content via `mint_from_intent` (TRUSTED), relying only on
session-level I0 demotion, so the *value* is not tainted the way M7 demands; (2) **the
neutralizer is private + git.push-scoped** — the viewer needs it shared and applied to *every*
displayed literal; (3) **no read-only viewer verb exists** and `print_audit_dag` is welded to a
live run, not a standalone session lookup; (4) **`load_or_create_key` CREATES a key when
absent** and returns a fresh key for `:memory:` — the viewer must instead fail CLOSED on an
absent key (U1 M2), so it needs a load-ONLY path.

**Primary recommendation:** Extend the existing `caprun` binary with two new verbs —
`caprun run …` (formalizing today's positional intent-run with an explicit `--policy` flag and
a post-run Block-surfacing line) and `caprun audit <session_id> <audit-db-path>` (read-only
viewer). Extract `neutralize_control_chars` into a shared module and apply it universally in the
viewer. Enforce M7 structurally by keeping the SDK's operator-literal intent path (→
`mint_from_intent`) disjoint from all file/stream/env ingestion (→ worker `mint_from_read`
only). No new crate, no library, no framework — manual-ops-first.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SDK-01 | Thin CLI/SDK: define intent → point at workspace → run end-to-end; extends existing verbs; binds trusted policy at session creation (POLICY-03); surfaces blocked `effect_id` + `caprun review` pointer on I2 Block; file/stream/env content minted TAINTED, not laundered (M7) | Run path + policy binding already wired (main.rs:276-389); Block surfacing gap + M7 laundering gap identified below |
| U1 (VIEW-01) | Read-only audit-DAG viewer over the SQLite chain; renders events/decisions + `verify_chain`; reuses exact `load_or_create_key` custody + F1 refusal; fails closed if key absent (never fresh/`:memory:`); control-char-neutralizes all tainted literals before display | `verify_chain`/`query_events_by_session` read-only anchors verified; neutralizer exists but private+git.push-scoped; load-only key gap identified |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| `caprun run` entrypoint (define→point→run) | CLI (cli/caprun binary) | Broker (session create + policy bind) | Orchestration only; binds policy, spawns broker+worker — no TCB decision logic |
| Trusted policy binding (POLICY-03) | Broker (`brokerd::policy::bind_policy`) | CLI (passes path) | Already broker-side + F1-contained; CLI only forwards the trusted path |
| I2-Block effect_id surfacing | CLI (post-run query) | Broker (audit DB rows) | Read-only presentation of an already-durable pending_confirmations row |
| Read-only audit viewer | CLI (new verb) | Broker (`audit::query_events_by_session`, `verify_chain`) | Pure read over SQLite; no worker, no sink, no state transition |
| MAC-key custody reuse | CLI (`key::load_or_create_key`) | adapter-fs (`containment::refuse_if_beneath_workspace`) | Same F1 choke-point both custody + policy already share |
| Tainted-byte display neutralization | shared helper (new locus) | CLI viewer + brokerd confirmation | Must be one implementation both the confirm prompt and the viewer call |
| Tainted-ingest value minting (M7) | Broker (`quarantine::mint_from_read`) | CLI (routes file content ONLY through worker read path) | Taint must be minted at the existing sole taint-mint site, never laundered via `mint_from_intent` |

## Verified Live-Code Anchor Table

All line numbers verified against current HEAD (`89cab31`, commits through Phase 44).

| # | Anchor | file:line | What it is | Disposition |
|---|--------|-----------|------------|-------------|
| A1 | verb dispatch (`confirm`/`deny`/`review`) | `cli/caprun/src/main.rs:75-107` | First branch, distinct arg shape, `exit()`s explicitly | **Reusable** — extend with `run`/`audit` verbs alongside |
| A2 | `grant` verb dispatch | `cli/caprun/src/main.rs:117-145` | Session-scoped verb, own helper `run_grant` | **Reusable pattern** for new verbs |
| A3 | run path: policy binding (POLICY-03) | `cli/caprun/src/main.rs:276-294` | `CAPRUN_POLICY` env → `bind_policy(path, workspace_root_dir)` at session creation | **Reusable** — SDK-01 adds `--policy` flag over this env hook |
| A4 | run path: MAC-key custody | `cli/caprun/src/main.rs:268-274` | `key::load_or_create_key(&audit_path, workspace_root_dir)` | **Reusable verbatim** |
| A5 | run path: worker spawn env | `cli/caprun/src/main.rs:454-485` | `env_clear()` + allowlist (`PATH`,`BROKER_SOCK`,`WORKSPACE_FILE`,`INTENT`) | **Reusable** — no change |
| A6 | run path: `--seed-from-file` | `cli/caprun/src/main.rs:157-196` | Reads file → `SeedProvenance::FileDerived` (Draft session) but value → `mint_from_intent` (TRUSTED) | **Needs-attention** — M7 laundering gap (WG-1) |
| A7 | audit-DAG print (run-scoped) | `cli/caprun/src/main.rs:513-522, 699-738` | `print_audit_dag(conn, session_id)` + `verify_chain` — bound to a live run's open conn | **Reference** — viewer needs a standalone open-by-path variant (WG-3) |
| A8 | `load_or_create_key` | `cli/caprun/src/key.rs:60-93` | `pub(crate)`; calls `adapter_fs::containment::refuse_if_beneath_workspace` (key.rs:78); `:memory:`→fresh ephemeral (key.rs:64); **CREATES** key if absent (key.rs:90) | **Reusable but** needs load-ONLY sibling for viewer fail-closed (WG-4) |
| A9 | F1 containment helper (extracted) | `crates/adapter-fs/src/containment.rs:refuse_if_beneath_workspace` | Shared, unit-tested; Phase 42 MAJOR-2 extraction from key.rs | **Reusable verbatim** (viewer key-load already gets it transitively) |
| A10 | `verify_chain` | `crates/brokerd/src/audit.rs:1240` | `pub fn verify_chain(conn, session_id, key: &[u8]) -> bool`; read-only; HMAC + chain_anchor MAC + orphan guard | **Reusable verbatim** — the viewer's core |
| A11 | `query_events_by_session` | `crates/brokerd/src/audit.rs:1056` | `pub fn … -> Result<Vec<Event>>`; ordered by rowid; read-only | **Reusable** — viewer's event source |
| A12 | `current_chain_head` / `find_event_by_id` | `crates/brokerd/src/audit.rs:1171 / 1147` | read-only accessors | **Reusable** if viewer walks the chain |
| A13 | `neutralize_control_chars` | `crates/brokerd/src/confirmation.rs:575` | Private `fn`; escapes every `char::is_control()` to `\xNN`/`\u{NNNN}`; pure | **Needs extraction** — private + only called for git.push (WG-2) |
| A14 | `render_block_display` | `crates/brokerd/src/confirmation.rs:676` | `pub fn`; shared by confirm/deny/review; neutralizes ONLY `if pc.sink.0 == "git.push"` (line 718) | **Reference** — viewer must neutralize for ALL sinks |
| A15 | `review` verb (read-only precedent) | `crates/brokerd/src/confirmation.rs:777` | Read-only, no state transition, no sink, no MAC gate | **Reusable pattern** for the viewer's read-only posture |
| A16 | `bind_policy` | `crates/brokerd/src/policy.rs:66` | `pub fn bind_policy(Option<&Path>, &Path) -> Result<(SessionPolicy, String)>`; `None`→`broker_default()`; containment-first | **Reusable verbatim** |
| A17 | mint sites | `crates/brokerd/src/quarantine.rs` | `mint_from_intent`:466 (TRUSTED), `mint_from_read`:301 (TAINTED, sole taint-mint site), `mint_from_http`:901, `mint_from_exec`:838 | **Reference** — M7 must route file/env content through `mint_from_read` only |
| A18 | ProvideIntent → mint | `crates/brokerd/src/server.rs:2370-2493` | ProvideIntent arm mints intent args via `mint_from_intent` (TRUSTED) regardless of `seed_provenance` | **Confirms WG-1**: file-derived intent param is minted trusted |
| A19 | `ExecutorDecision` + `SinkBlockedAnchor.effect_id` | `crates/runtime-core/src/executor_decision.rs:265, 211` | `BlockedPendingConfirmation { anchors: Vec<BlockedArg> }`; each `anchor.effect_id: Uuid` | **Reusable** — the effect_id to surface |
| A20 | worker Block handling | `cli/caprun/src/worker.rs:395-400` | On non-Allowed: `eprintln!("[worker] NOT ALLOWED ({decision:?})")` + `exit(1)` — Debug dump on worker stderr, NOT surfaced by parent | **Needs-attention** — Block-surfacing gap (WG-6) |

## The CLI/SDK Shape Recommendation (SDK-01)

**Shape:** A **new `run` subcommand on the existing `caprun` binary** — NOT a separate library
crate, NOT a framework. This satisfies "thin CLI/SDK… manual-ops-first, no framework" and
"extends, does not replace" the existing verbs with the least new surface.

**Why not a library crate:** `LIVE-05` requires the composed proof be *driven* via this CLI on
real Linux; a binary subcommand is directly drivable from `compose-verify.sh`/tests, a library
is not. The `run_grant`/`run_confirm_or_deny` helpers (main.rs:545,650) are the established
"parse args → open DB → call into brokerd → map to exit code" pattern to mirror.

**Concretely:**
1. Add a `run` verb to the top-of-`main` dispatch (alongside confirm/deny/review/grant,
   main.rs:75-146). Today the intent-run is the *fall-through* positional path (main.rs:148+);
   promote it to an explicit `caprun run` verb so the surface is legible, while keeping the
   bare-positional form working for back-compat (existing e2e tests pass no verb).
2. Add an explicit **`--policy <path>` flag** that feeds `bind_policy` (A3/A16), formalizing
   the `CAPRUN_POLICY` env hook (which stays as a fallback). This is the SDK-01 sentence "the
   run entrypoint takes the trusted policy path and binds it at session creation" — the Track-3
   → Track-4 connection. **The binding is already correct and F1-contained (A3);** the flag is
   pure surface.
3. Intent definition stays operator-typed literals only (see M7 guarantee below).

**Minimal viable form:**
`caprun run <intent-kind> <intent-param> <workspace-file> [--policy <path>] [audit-db-path]`

## The I2-Block Operator Loop (Matt #2)

**Current state (WG-6):** On an I2 Block the worker prints `[worker] NOT ALLOWED
(BlockedPendingConfirmation { … })` to its OWN stderr and exits 1 (worker.rs:395-400). The
parent `caprun` process then prints the full audit DAG and `caprun-worker exited with status…`
(main.rs:526). The blocked `effect_id` IS durably in the DB (the `sink_blocked` event + the
`pending_confirmations` row) and IS present inside the worker's Debug dump, but the **parent
entrypoint never surfaces it as an actionable `caprun review <effect_id>` pointer** — the
operator has to grep the Debug output or the DAG.

**Recommendation:** After the worker exits non-zero (main.rs:526), query the session's
`pending_confirmations` for any `Pending` row(s) and print, to the parent's stdout, a clear
actionable block:

```
BLOCKED (pending confirmation): effect_id <uuid>  sink <sink>
  Inspect:  caprun review  <uuid> <audit-db-path>
  Release:  caprun confirm <uuid> <audit-db-path>
  Refuse:   caprun deny    <uuid> <audit-db-path>
```

Read the pending rows via the existing `find_pending_confirmation` shape (confirmation.rs:391)
or a small `list_pending_for_session` query. This is **read-only presentation** of an
already-durable row — no new TCB, no state change. It closes Matt #2 ("makes the loop actually
design-partner-runnable").

## The Viewer Shape + Fail-Closed-on-Absent-Key (U1)

**Verb:** `caprun audit <session_id> <audit-db-path>` (read-only). Named `audit` (not `view`) to
mirror the artifact it renders; `<audit-db-path>` is **required, not defaulted to `:memory:`** —
unlike confirm/deny (main.rs:95-98), a `:memory:` DB has no persisted chain to inspect, so
defaulting it would be a meaningless verdict (U1 M2 forbids exactly this).

**Output format (terminal, no web UI):**
1. Header: session_id, event count.
2. The causal DAG walk — reuse `print_audit_dag`'s recursive-CTE ordering (A7, main.rs:699) but
   parameterized on an open-by-path connection; render each event's `depth`, `event_type`,
   `actor`, short hash/parent — **every displayed field neutralized** (WG-2).
3. Per-effect decision lines (Allowed / Blocked-pending / Denied / policy-deny), read from event
   types + `pending_confirmations`.
4. Final line: `Chain verification: PASSED|FAILED` from `verify_chain(&conn, session_id, &key)`
   (A10) — the exact call `caprun run` already makes at main.rs:517.

**Read-only posture:** Model on `review` (A15) — never transitions state, never appends an
event, never opens the workspace root, never invokes a sink/executor. Pure read.

**Fail-closed-on-absent-key mechanism (U1 M2) — the security-load-bearing part:**
`load_or_create_key` (A8) is the wrong primitive as-is because it **creates** a key when the
`.key` sibling is absent (key.rs:90) and returns a **fresh** key for `:memory:` (key.rs:64) —
both would produce a green-but-meaningless `verify_chain` verdict against a chain it never
signed. The viewer MUST instead:
- **Refuse `:memory:`** at arg-parse (see above).
- Load the key via a **load-ONLY path**: check `<audit-db-path>.key` exists; if absent →
  **hard error, refuse to render a verdict** (exit non-zero), NEVER generate one. Recommended:
  add a sibling `load_existing_key(audit_path, workspace_root) -> Result<Vec<u8>>` in key.rs
  that runs the SAME F1 `refuse_if_beneath_workspace` containment check (A9) then reads the
  existing file, erroring if absent — i.e. `load_or_create_key` minus the create branch. The
  viewer calls THIS, so an absent key is fail-closed by construction.
- Keep the F1 containment refusal (A9) so a viewer pointed at an audit DB beneath the workspace
  root is refused exactly as custody is — "out of the confined worker's reach."

**Note:** the viewer verb lives in the `cli/caprun` binary, same crate as `key.rs`, so
`pub(crate)` visibility (A8) is sufficient — no cross-crate extraction needed for the key
(unlike the neutralizer, WG-2).

## The SDK Tainted-Ingest Structural Guarantee (M7)

**The gap (WG-1):** Today `--seed-from-file` (A6) reads file content and hands it to the intent
as `intent_param`; the broker's ProvideIntent arm mints it via `mint_from_intent` — the TRUSTED
mint (A17/A18). The only safety is session-level `SeedProvenance::FileDerived` → Draft (I0),
which stops *auto-authorization* but does NOT taint the *value*. M7 demands the value itself be
minted TAINTED (draft-only per I0/I1), "exactly like any other raw read."

**Recommended mechanism — structural disjointness, no new mint verb:**
Keep the SDK's intent-construction path (operator-typed literals) **disjoint from all
file/stream/env ingestion**. Concretely:
- The `caprun run` intent args accept **operator-typed literals only** → `mint_from_intent`
  (TRUSTED) — this is genuinely operator-typed, correct per M7's first clause.
- Any file/stream/env content the operator wants in play flows **exclusively through the worker's
  existing `RequestFd → read_within → mint_from_read` path** (A17, the sole taint-mint site),
  which taints by construction. The SDK has **NO API that reads a file and passes its bytes to
  ProvideIntent** — that is the structural guarantee: by construction the SDK cannot reach the
  trusted mint with ingested bytes.
- **Therefore: treat `--seed-from-file`'s trusted-value mint as the thing M7 closes.** RECOMMEND
  the planner either (a) restrict `--seed-from-file` so the file-derived value is routed through
  a tainted mint rather than `mint_from_intent`, or (b) if that value is only ever a session-seed
  (never a sink arg), keep the I0 demotion but add an explicit assertion/test that a
  file-seeded value can never satisfy a sensitive sink arg without an I2 Block. Option (a) is the
  stronger M7 guarantee; **prefer (a)** if the file-derived param can reach any sink arg. This is
  the one place in Phase 45 that may touch broker mint wiring — flag for the DESIGN/plan-checker
  to confirm which of (a)/(b) matches the shipped `--seed-from-file` semantics.

**Why not a new `mint_from_ingest` broker verb:** that is new TCB taint-mint surface and would
re-open the design gate. The disjointness approach reuses `mint_from_read` (already the sole,
audited taint-mint site) and needs no new TCB mint.

## Control-Char Neutralization Reuse (U1 M3)

**The helper exists and is correct** (A13, confirmation.rs:575): `neutralize_control_chars`
escapes every `char::is_control()` byte (C0 incl. ESC/CR/LF/TAB, C1 range, DEL) to a visible
`\xNN`/`\u{NNNN}`, preserving ordinary printable + non-ASCII UTF-8. Its doc comment already says
it "mirrors the U1 / VIEW-01 viewer discipline" — the design anticipated this reuse.

**Two problems for reuse (WG-2):**
1. It is a **private `fn`** in `confirmation.rs` — not callable from a new viewer module.
2. `render_block_display` applies it **only for git.push** (`if pc.sink.0 == "git.push"`,
   confirmation.rs:718). A tainted email body / POST body / commit message from any OTHER sink is
   currently displayed byte-verbatim in the confirm prompt (accepted there under T-10-04's
   verbatim-display policy). The **viewer** has no such verbatim mandate and MUST neutralize
   **every** displayed literal regardless of sink (U1 M3: "All tainted literal bytes").

**Recommendation:** **Extract `neutralize_control_chars` into ONE shared, unit-tested helper**
(mirror the exact pattern of the Phase 42 MAJOR-2 F1 extraction, A9) — placed in a crate
reachable by BOTH `crates/brokerd::confirmation` and the `cli/caprun` viewer. Since `brokerd` is
already a `cli/caprun` dependency, a `pub fn` in a small `brokerd` util module (e.g.
`brokerd::display::neutralize_control_chars`) is the lowest-friction locus; `runtime-core` also
works. Then:
- `confirmation.rs` calls the shared fn (behavior unchanged — still git.push-scoped there).
- The viewer calls the shared fn on **every** rendered field (`actor`, event literals, any
  displayed payload-derived string). A regression test asserts both sites call the shared fn
  (anti-drift, mirroring the F1 shared-fn test).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Verify the audit hash chain | A re-implemented HMAC walk | `audit::verify_chain` (A10) | Already does chain-linkage + constant-time MAC + chain_anchor MAC + orphan guard; re-rolling risks a weaker check |
| Read a session's events | A raw SQL query in the viewer | `audit::query_events_by_session` (A11) | Deserializes payloads to typed `Event`, ordered; one source of truth |
| Neutralize terminal control chars | A new escape routine in the viewer | Extracted `neutralize_control_chars` (WG-2) | Two copies drift; the existing one is proven + tested |
| F1 containment for the key/policy path | A new path check | `adapter_fs::containment::refuse_if_beneath_workspace` (A9) | The ONE shared choke-point custody + policy already share |
| Load the MAC key | A hand-rolled key reader | `key::load_or_create_key` + a load-ONLY sibling (A8/WG-4) | Cross-process custody + F1 already correct; only the create-vs-fail-closed branch differs |
| Taint file/env content | A new broker mint verb | Existing `mint_from_read` worker path (A17) | Sole audited taint-mint site; new verb re-opens the design gate |

**Key insight:** Phase 45 is a *reuse-and-surface* phase. Every security-relevant primitive
(verify_chain, F1 containment, the neutralizer, the mint sites) already ships and is tested; the
failure mode is re-implementing one of them slightly weaker, not missing one.

## Common Pitfalls

### Pitfall 1: Viewer silently mints a fresh key and reports a green chain
**What goes wrong:** Viewer calls `load_or_create_key`, which creates a `.key` if absent
(key.rs:90) or returns a fresh `:memory:` key (key.rs:64); `verify_chain` then verifies a chain
against a key that never signed it → meaningless (possibly PASSED-looking) verdict.
**How to avoid:** Load-ONLY key path (WG-4); refuse `:memory:`; hard-error on absent `.key`.
**Warning signs:** A `.key` file appearing after running the viewer on a DB that had none.

### Pitfall 2: Neutralizing only git.push literals in the viewer
**What goes wrong:** Copying `render_block_display`'s `if sink == "git.push"` guard
(confirmation.rs:718) into the viewer leaves a tainted email/POST/commit body un-neutralized →
ANSI/CR spoofing of audit lines (the exact U1 M3 surface).
**How to avoid:** Viewer neutralizes EVERY displayed literal unconditionally.
**Warning signs:** A test with a tainted body containing `\x1b[2K` renders as a cleared line.

### Pitfall 3: Laundering file content through the trusted intent mint (M7)
**What goes wrong:** SDK reads a file → passes bytes as an intent param → `mint_from_intent`
(TRUSTED); an I2 Block never fires because the value isn't tainted.
**How to avoid:** Disjoint operator-literal intent path from file/env ingestion (M7 section).
**Warning signs:** A file-sourced value with `UserTrusted` taint reaching a sensitive sink arg.

### Pitfall 4: Viewer transitions state or holds an fd
**What goes wrong:** Viewer accidentally opens the workspace root or appends an event, becoming
an authority surface.
**How to avoid:** Model on `review` (A15) — read-only, no workspace root, no sink, no event.
**Warning signs:** The viewer taking a `WorkspaceRoot` or a `&mut Connection`.

## Runtime State Inventory

Phase 45 is additive CLI surface + one helper extraction — it renames/migrates nothing.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — viewer only READS the existing `events`/`pending_confirmations`/`chain_anchor` tables (audit.rs:100-202) | none |
| Live service config | None — no external service touched | none |
| OS-registered state | None | none |
| Secrets/env vars | `CAPRUN_POLICY` (existing, A3) gains a `--policy` flag alias; MAC key `.key` sibling read (not written) by viewer | code only (add flag, add load-only reader) |
| Build artifacts | New `caprun run`/`caprun audit` verbs compile into the SAME `caprun` binary; `neutralize_control_chars` extraction moves a private fn to a shared module | rebuild workspace; anti-drift test |

## Wiring Gaps (highest-value output)

**WG-1 — M7 laundering: file-derived intent value is minted TRUSTED.** `--seed-from-file`
(main.rs:157-196) → intent param → ProvideIntent → `mint_from_intent` (TRUSTED, server.rs:2438).
Session is demoted to Draft (I0) but the *value* is not tainted. M7 requires ingested content
minted TAINTED. **Fix:** route file/stream/env content only through `mint_from_read`; keep the
operator-literal intent path disjoint (M7 section). Confirm with the planner whether the
file-derived seed value can reach a sink arg (→ needs the tainted mint) or is seed-only (→ I0 +
an assertion suffices). **This is the one gap that may touch broker mint wiring — flag for the
design/plan-checker.**

**WG-2 — neutralizer is private + git.push-scoped.** `neutralize_control_chars`
(confirmation.rs:575) is a private `fn`; `render_block_display` applies it only for git.push
(confirmation.rs:718). The viewer (U1 M3) needs it (a) callable cross-module and (b) applied to
EVERY displayed literal. **Fix:** extract into a shared `pub fn` (mirror the F1 extraction, A9),
apply universally in the viewer, add an anti-drift test.

**WG-3 — no standalone read-only viewer verb; `print_audit_dag` is run-welded.**
`print_audit_dag` (main.rs:699) takes a live run's open `conn` and is called only inside a run
(main.rs:516). There is no `caprun audit <session>` verb that opens an arbitrary DB by path and
renders it. **Fix:** new read-only verb + an open-by-path variant of the DAG walk + `verify_chain`.

**WG-4 — `load_or_create_key` creates/fresh-mints; viewer needs load-only fail-closed.**
key.rs:64 (`:memory:`→fresh) and key.rs:90 (create-if-absent) both defeat U1 M2's
fail-closed-on-absent-key requirement. **Fix:** add `load_existing_key` (F1 check + read
existing, error if absent); viewer refuses `:memory:` and calls the load-only path.

**WG-5 — I2-Block effect_id not surfaced by the parent entrypoint.** On Block the worker prints
Debug to its own stderr + exits 1 (worker.rs:395); the parent prints the DAG + exit status but
no actionable `caprun review <effect_id>` pointer (main.rs:526). **Fix:** parent queries
`pending_confirmations` post-run and prints the effect_id + review/confirm/deny pointers (Matt #2
section). Read-only, no TCB.

**WG-6 (minor) — policy surface is env-only.** Run path binds policy from `CAPRUN_POLICY` env
(main.rs:289); SDK-01 formalizes a CLI surface. **Fix:** add `--policy <path>` flag mapping to
the same `bind_policy` call. Functionally already wired (A3); pure surface.

## Suggested Plan Breakdown

Mirrors the project's wave/dependency convention (e.g. Phase 42's `01+02 → 03 → 04`).
**4 plans, 3 waves.** This phase is on the acceptance critical path (Phase 46 LIVE-05 drives the
composed proof through these verbs), so the acceptance plan must prove genuine end-to-end use.

- **45-01 (Wave 1) — SDK-01 run entrypoint + Block surfacing + M7 guarantee.** Promote the
  positional intent-run to a `caprun run` verb; add `--policy <path>` over `bind_policy` (WG-6);
  add post-run Block surfacing of `effect_id` + review pointer (WG-5); enforce M7 structural
  disjointness (WG-1) — decide (a) tainted-mint routing vs (b) seed-only + assertion with the
  design/plan-checker. [SDK-01]
- **45-02 (Wave 1) — extract `neutralize_control_chars` into a shared `pub fn` + anti-drift
  test.** Rewire `confirmation.rs` to the shared fn (behavior unchanged); foundation for the
  viewer. Independent of 45-01. [U1 M3 foundation, WG-2]
- **45-03 (Wave 2, depends on 45-02) — `caprun audit <session_id> <audit-db-path>` read-only
  viewer.** Open-by-path DAG walk + `verify_chain` verdict; `load_existing_key` load-only
  fail-closed (WG-4); refuse `:memory:`; neutralize every displayed literal (WG-2); model on
  `review`'s read-only posture. [U1, WG-3]
- **45-04 (Wave 3, depends on 45-01 + 45-03) — acceptance test.** End-to-end: `caprun run` →
  I2 Block → surfaced effect_id → `caprun review` → `caprun audit` renders events/decisions +
  `verify_chain` true; plus fail-closed-on-absent-key negative, `:memory:`-refused negative, and
  a tainted-body-neutralization assertion. Sets up the LIVE-05 driver. [SDK-01, U1]

## Environment Availability

Phase 45 is CLI/code-only on the shipped stack — no new external dependency. Note the standing
constraint (CLAUDE.md): all security-relevant *acceptance* runs go through
`scripts/mailpit-verify.sh` on real Linux (Colima+Docker); macOS `cargo test` compiles no
`#[cfg(target_os="linux")]` targets. The viewer + CLI logic itself is host-testable (pure reads,
no Landlock/seccomp), but the composed drive belongs on Linux.

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Existing `caprun` binary + brokerd/audit/key modules | all of Phase 45 | ✓ (in-repo) | HEAD `89cab31` | — |
| rusqlite (audit reads) | viewer | ✓ (workspace dep) | shipped | — |
| Colima/Docker + mailpit-verify.sh | LIVE acceptance drive | ✓ (per CLAUDE.md) | — | host cargo test for logic only |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `cargo test` (workspace) |
| Config file | none (Cargo workspace, `resolver="3"`) |
| Quick run command | `cargo test -p caprun --lib` (viewer/CLI unit tests, host-OK) |
| Full suite command | `bash scripts/mailpit-verify.sh` (real Linux; default `cargo test --workspace --no-fail-fast`) |

### Phase Requirements → Test Map
| Req | Behavior | Test Type | Command | Exists? |
|-----|----------|-----------|---------|---------|
| SDK-01 | `caprun run` binds policy + surfaces Block effect_id | integration | `cargo test -p caprun --test e2e` (extend) | ❌ Wave 0 (45-04) |
| SDK-01 M7 | file/env content minted tainted, not laundered | unit/integration | new test asserting file-seed value can't satisfy a sink arg without I2 Block | ❌ Wave 0 (45-01/04) |
| U1 | viewer renders events + `verify_chain` verdict | integration | new `cli/caprun/tests/audit_viewer.rs` | ❌ Wave 0 (45-03) |
| U1 M2 | fail closed on absent key; never `:memory:`/fresh | unit | key `load_existing_key` refusal test | ❌ Wave 0 (45-03) |
| U1 M3 | tainted literal control-char-neutralized in viewer | unit | shared-neutralizer test + viewer render test | ❌ Wave 0 (45-02/03) |

### Sampling Rate
- **Per task commit:** `cargo test -p caprun --lib`
- **Per wave merge:** `cargo build --workspace && cargo test -p caprun`
- **Phase gate:** `bash scripts/mailpit-verify.sh` green on real Linux before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `cli/caprun/tests/audit_viewer.rs` — U1 render + verify_chain + fail-closed-on-absent-key
- [ ] Extend `cli/caprun/tests/e2e.rs` — SDK-01 run verb + Block surfacing
- [ ] Shared-neutralizer unit test + anti-drift (both call sites) — U1 M3

## Security Domain

`security_enforcement` treated as enabled (absent in config = enabled). Phase 45 is not deep TCB
but touches two security-relevant surfaces: MAC-key custody (F1) and tainted-byte display.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V5 Input Validation / Output Encoding | yes | `neutralize_control_chars` on ALL displayed tainted bytes (U1 M3) — terminal-escape neutralization is output encoding |
| V6 Cryptography | yes | Reuse shipped HMAC-SHA256 `verify_chain` + `getrandom` key custody — never re-roll (Don't Hand-Roll) |
| V4 Access Control | yes | Viewer is read-only + F1-contained + out of confined-worker reach; load-only key fail-closed |
| V7 Error Handling / Logging | yes | Fail-closed on absent key / unresolvable path / `:memory:` — no fail-open verdict |

### Known Threat Patterns
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Terminal ANSI/CR audit-line spoofing via tainted literal | Tampering / Repudiation | Universal `neutralize_control_chars` in viewer (WG-2, U1 M3) |
| Meaningless green `verify_chain` from a fresh/`:memory:` key | Spoofing / Repudiation | Load-only key, fail closed on absent (WG-4, U1 M2) |
| Worker widens its own policy via a workspace-reachable policy file | Elevation of Privilege | POLICY-03 F1 refusal already shipped (A9/A16) — reused verbatim |
| Provenance laundering: file content minted trusted | Tampering | M7 disjoint operator-literal vs `mint_from_read` (WG-1) |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `--seed-from-file`'s file-derived value can reach a sink arg (so M7 needs a tainted mint, option (a)) OR is seed-only (option (b) suffices) — unresolved which | M7 / WG-1 | If it can reach a sink arg and stays trusted-minted, M7 is unmet; the planner must confirm against the shipped ProvideIntent→sink flow |
| A2 | A `pub fn` in a `brokerd` util module is an acceptable shared locus for the extracted neutralizer (brokerd is already a cli/caprun dep) | WG-2 | If brokerd shouldn't host display code, use `runtime-core` instead — low risk, either works |

**All other claims are `[VERIFIED: codebase]`** — read directly from HEAD `89cab31`.

## Sources

### Primary (HIGH confidence)
- `cli/caprun/src/main.rs` (verb dispatch, run path, policy binding, key custody, print_audit_dag) — read in full
- `cli/caprun/src/key.rs` (load_or_create_key, F1 delegation, :memory: + create branches) — read in full
- `cli/caprun/src/worker.rs:360-403` (Block/decision handling)
- `crates/brokerd/src/audit.rs` (schema, query_events_by_session, verify_chain, chain accessors)
- `crates/brokerd/src/confirmation.rs:540-784` (neutralize_control_chars, render_block_display, review)
- `crates/brokerd/src/policy.rs:60-90` (bind_policy signature + broker_default branch)
- `crates/brokerd/src/quarantine.rs` (mint_from_intent/read/http/exec locations)
- `crates/brokerd/src/server.rs:2370-2493` (ProvideIntent → mint_from_intent)
- `crates/adapter-fs/src/containment.rs` (refuse_if_beneath_workspace — shared F1 helper)
- `crates/runtime-core/src/executor_decision.rs:208-287` (SinkBlockedAnchor.effect_id, ExecutorDecision)
- `planning-docs/DESIGN-v1.9-egress-policy.md` §5.3 (POLICY-03 binding), §6 (threat model)
- `.planning/ROADMAP.md` Phase 45 (goal + 4 success criteria), `.planning/REQUIREMENTS.md` (SDK-01, U1 full text)

## Metadata

**Confidence breakdown:**
- Live-code anchors: HIGH — every line re-verified against HEAD `89cab31`
- CLI/SDK + viewer shape: HIGH — extends established verb-dispatch + read-only `review` patterns
- Wiring gaps: HIGH — each traced to an exact line; WG-1 (M7) carries one open decision (A1)
- Plan breakdown: MEDIUM — mirrors project convention; exact plan count is the planner's call

**Research date:** 2026-07-18
**Valid until:** 2026-08-17 (stable; invalidate earlier if Phase 43/44 land follow-up edits to confirmation.rs or key.rs)
