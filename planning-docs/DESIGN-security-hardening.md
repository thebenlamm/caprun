# DESIGN — Security Hardening (close the v1.6 TCB-local residuals)

**Milestone:** v1.6 — Security Hardening
**Phase:** 26 (Design Gate) — hard-blocks all `crates/executor` / `crates/brokerd` / `crates/runtime-core` hardening code for this milestone (Phases 27–30)
**Status:** Draft → pending fresh (non-self) adversarial review (see `DESIGN-GATE-RECORD-v1.6.md`)
**Author date:** 2026-07-12
**Requirements gated by this doc:** DESIGN-11 (this doc exists and pins mechanism + fail-closed default for all five residuals), DESIGN-12 (this doc clears a fresh non-self adversarial review with every finding resolved)
**Grounding:** `.planning/phases/26-security-hardening-design-gate/26-RESEARCH.md` — every file:line below was re-verified against live source at authoring time (greps re-run per the research's "Valid until" clause); re-verify again at Phase 27+ if many commits intervene.

> **Design-gate discipline.** No `crates/executor`, `crates/brokerd`, or `crates/runtime-core`
> hardening code may be written until this document clears a fresh, non-self adversarial review with
> every finding resolved — mirroring v1.0 Phase 2, v1.2 Phase 8, v1.3 Phase 12, v1.4 Phase 18, and
> v1.5 Phase 23. This doc pins **decisions**, not options; Phases 27–30 are a mechanical realization
> of what §a–§e and §f fix.

---

## §0. Purpose & Scope

**The gap (v1.5 exit residuals).** v1.5 shipped slot-type binding (T2) and closed the last
degree-of-freedom in value routing. The milestone-closure audit and the v1.3 Phase-16 v2-security
panel left **five TCB-local residuals** — each a place where the reference monitor's own machinery
can be sidestepped without crossing the kernel boundary or defeating I2. None of the five adds new
external-effect surface; all five are hardening of mechanisms that already exist:

1. **§a — HARDEN-01:** a silent/injected worker skips the only I1 demotion site by simply not
   sending `ReportClaims`, so `RequestFd` alone (reading untrusted bytes) carries no draft-only
   consequence.
2. **§b — HARDEN-02:** `verify_chain` over an **unkeyed** SHA-256 chain lets an in-host `events`-table
   writer forge or truncate history undetectably.
3. **§c — HARDEN-03:** the trusted `email.send` Allowed path has no at-most-once guard, so a replayed
   `SubmitPlanNode` sends N times.
4. **§d — HARDEN-04:** the forced-`Active` `CreateSession` mint is only **runtime**-gated
   (`CAPRUN_ENABLE_IPC_CREATE_SESSION`), so the bypass code physically ships in the release binary.
5. **§e — HARDEN-05:** `file.create`'s `contents` slot is role-**unconstrained**, so a future
   tainted file-body value would not route into the I2 collect-then-Block path.

**Threat ceiling (locked, D-04).** v1.6 defends against an **in-host DB-writer** (an actor with
`events`/store write access on the host) and a **statically-compromised or silent worker** — NOT a
full host/root compromise that can read the broker's key. The out-of-scope stronger adversary is
recorded as a named Accepted Residual Risk (§ Accepted Residual Risks, D-05), not silently claimed.

**Cross-cutting rulings.** Three questions cut across the five residuals (label continuity X-01,
shared-store recovery authority X-02, TOCTOU atomic ordering X-03) and a fourth (X-04) surfaced by
the research as a NEW code-traced finding no locked decision named. §f pins one uniform rule for each
and rules explicitly on X-04.

**Scope discipline.** This is a **decisions doc, not an options survey** — mirroring
`planning-docs/DESIGN-slot-type-binding.md` (v1.5). Every § pins the exact current-code anchor, the
mechanism, the fail-closed default, the false-positive surface, and an informative Phase-27+
blast-radius note. All five mechanisms stay **hardcoded in the Rust TCB** — no swappable policy file,
no config surface (`CON-i2-non-bypassable`; `sink_sensitivity.rs`'s own "a security property, not a
configuration knob" doc comment is the pattern to keep matching). `check-invariants.sh` (Gate 1 no
`EffectRequest`, Gate 3 mint-call-site restriction) is the compile/CI backstop for every phase.

**Explicitly out of scope (locked, `.planning/REQUIREMENTS.md` Out-of-Scope + CONTEXT.md Deferred):**
- Full host/root-compromise tamper-evidence (external out-of-store notarization) — D-05, later milestone.
- A per-session effects-budget / send rate-limit — D-08 defense-in-depth beyond per-plan-node CAS.
- Full output-file provenance labeling (xattr/sidecar) for `file.create` `contents` — D-12; v1.6 uses
  input-role treatment + the X-01 label-continuity fail-closed rule instead.
- v1.7 breadth: Git/GitHub/test/patch-PR/snapshot adapters.

**File:line re-verification note.** Because Phases 27–30 land immediately after this gate, staleness
risk is low; nonetheless each § carries the anchor confirmed at authoring, and Phase 27+ MUST re-run
the key greps (`grep -n "CAPRUN_ENABLE_IPC_CREATE_SESSION" crates/brokerd/src/server.rs`,
`grep -n "session_status" crates/brokerd/src/server.rs`, `grep -n "contents" cli/caprun/src/planner.rs`,
`grep -n "contents" crates/executor/src/sink_sensitivity.rs`) before touching code if commits intervene.

---

## §a — HARDEN-01: demote-at-RequestFd (I1 honest scope)

**Requirement:** the `fd` release itself carries the I1 draft-only consequence — reading untrusted
bytes demotes, whether or not the worker later self-reports. **Lands Phase 27.**

### Current-code anchor (re-verified at authoring)

- `BrokerRequest::RequestFd` arm, `crates/brokerd/src/server.rs` (≈`:996-1047`): sets `*fd_requested = true`
  at entry (the broker-side, per-connection, RequestFd-entry-mutated flag D-02 cites as precedent —
  `fd_requested` is initialized `false` at `server.rs:414` and threaded `&mut` exactly like
  `session_status`), resolves the path under the workspace dirfd via
  `openat2(RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)`, appends a `fd_granted` Event
  (`let fd_event_id = Uuid::new_v4();` at `server.rs:1014`), and passes the fd via `SCM_RIGHTS`.
  **`session_status` is never read, matched, or mutated anywhere in this arm** (confirmed by direct read).
- The SOLE I1 demotion site today is `mint_from_read` (`crates/brokerd/src/quarantine.rs`), reached
  ONLY from `BrokerRequest::ReportClaims` (`server.rs`, the arm that runs
  `*session_status = SessionStatus::Draft` at `server.rs:1131`) — a message the possibly
  compromised/silent worker chooses whether to send. `mint_from_read` demotes atomically under the
  same lock as its `file_read` Event append: `update_session_status(conn, session_id, Draft)` (a plain
  `UPDATE sessions SET status=?1 WHERE id=?2`, no CAS guard — idempotent because `Draft` is monotonic)
  then an append-only `session_demoted` Event parented on the just-appended `file_read` id (TAINT-04's
  pinned causal edge).
- **No broker-side "trusted label" plumbing exists today.** `cli/caprun/src/main.rs:187` derives
  `workspace_rel` (the CLI's own `<workspace-file>` basename) but forwards it ONLY to the worker
  subprocess via `.env("WORKSPACE_FILE", workspace_rel)` (`main.rs:301`) — it is NEVER passed into
  `run_broker_server` (`main.rs:238`) / `dispatch_request`. The broker has no in-memory notion of
  "THE file the CLI designated as this session's intent document" distinct from any other in-workspace
  path the worker happens to `RequestFd`.

### Mechanism (pinned)

Add the demotion at `RequestFd`'s entry, symmetric with `*fd_requested = true`. Before the fd is
opened/passed, a broker-derived `is_trusted_labeled(path) -> bool` check runs; on `false`, demote via
the SAME atomic pattern `mint_from_read` already uses (`update_session_status` + causally-linked
`Draft`-triggering Event) **before** `read_within`/`pass_fd` runs.

- **D-01 fail-closed ordering (locked):** demotion **commits before the fd is released** to the worker.
  Worker-reported evidence may only DEMOTE; any keep-Active decision must be **broker-derived**.
- **D-03 trusted-label criterion (locked):** the stay-Active criterion shifts from "fragment-free" to
  **"trusted-labeled file."** `is_trusted_labeled`'s broker-derivable, non-parsing signal is a
  **path-identity compare**: does the `RequestFd`'d path equal the CLI's designated `<workspace-file>`
  (`workspace_rel`)? The compare MUST reuse the SAME `openat2(RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)`
  canonical form already computed for the fd-open — never a second ad-hoc string compare (a permissive
  normalization could treat an attacker lookalike path as trusted). **No content parsing at fd time**
  (TOCTOU).
- **Fail-closed default (locked):** an unlabeled path — or a `None`/absent trusted-path value — is
  `is_trusted_labeled == false` → **demote**. Availability cost (over-demoting a legitimate read) is
  the safe direction; under-demoting is not.

### RESEARCH CORRECTION (ii) — new plumbing budget, not a new `if`

The trusted-label signal has **no existing broker-side plumbing**. `workspace_rel` reaches only the
worker (`WORKSPACE_FILE` env). Phase 27 MUST budget a genuinely **NEW threading hop**: pass
`workspace_rel` (or an `Option<PathBuf>` "trusted document path") into `run_broker_server`
(signature at `server.rs:149`, `initial_session_status` param) and store it per-connection/per-session
alongside `workspace_root`, **mirroring how `session_status` / `fd_requested` are already threaded**
(seeded from `main.rs:238`'s call site). This is new state, not a rewire of an existing field — do not
plan it as merely a new branch inside the `RequestFd` arm.

### Causal-edge target (pinned)

Today `session_demoted.parent_id == file_read.id`. A fd-grant-time demotion has **no `file_read` Event
yet** — it MUST parent on **`fd_granted`'s id** (`fd_event_id`, `server.rs:1014`), or, if the demotion
is appended before `fd_granted` under the new ordering, on the **current chain head** immediately
before it. `DESIGN-session-trust-state.md` §5 does not anticipate this second causal shape; Phase 27
adds the case.

### D-02 reconciliation with `DESIGN-session-trust-state.md:80-81`

The pinned clause — *"No other function in `brokerd` MUST be permitted to set `SessionStatus::Draft`
for the I1 reason"* — is **reconcilable, not blocking.** The status quo actually **violates its own
anti-self-declaration rationale** (`:84-87`): `mint_from_read` is reached solely via the
worker-optional `ReportClaims` path, so a silent/injected worker skips demotion entirely and the
session stays falsely `Active` — exactly the spoofing the clause's rationale forbids. Relocating (adding)
the demotion to fd-grant is the **broker's own act** (precedent: `fd_requested` is flipped broker-side
at `RequestFd` entry today), never a worker-asserted flag — so the anti-self-declaration invariant is
**strengthened, not weakened.** Phase 27 MUST amend the pinned doc's **letter** to name the
`RequestFd`-entry demotion as a **second, broker-side, trusted-path-only I1 demotion site** (both
remain broker-only). It MUST also correct `mint_from_read`'s doc comment, which currently claims it is
the **SOLE I1 trust-flip site** — that sentence becomes false the moment §a lands and must be fixed in
the SAME Phase-27 PR, not left stale.

### Risk / false-positive surface

- **Double-demotion is safe but must stay idempotent.** `update_session_status`'s bare `UPDATE` is
  naturally idempotent; a second `session_demoted` Event (fd-grant demotion + a later `mint_from_read`
  on the same connection) is fine for audit legibility. Phase 27 states whether the second event is
  expected or deduplicated.
- **Path-normalization mismatch** (`./report.txt` vs `report.txt`): reuse the canonicalized fd-open
  form; a naive string `==` over-demotes (safe) or, if too permissive, under-demotes (dangerous).
- **Single-file scope:** `workspace_rel` is one file today. A future multi-file trusted flow must
  generalize `is_trusted_labeled` from "equals the one path" to "is in the trusted-path set" — a
  one-line forward note only; out of v1.6 live scope.

### Blast radius (Phase 27)

`server.rs`: `RequestFd` arm (new demotion before fd-open); `run_broker_server`/`handle_connection`/
`dispatch_request` signatures (new `trusted_path`/`workspace_rel` parameter, threaded like
`initial_session_status`). `quarantine.rs`: `mint_from_read` doc-comment correction.
`cli/caprun/src/main.rs`: second forwarding of `workspace_rel` into the `run_broker_server` call
(`:238`) alongside its existing `WORKSPACE_FILE` env forward. `planning-docs/DESIGN-session-trust-state.md`:
§2 amendment naming the new demotion site + §5 second causal-edge case. Tests: `s9_control_ab`
(CONTROL-01) must not regress; a NEW negative test — `RequestFd` on an untrusted (non-`workspace_rel`)
path with NO subsequent `ReportClaims` — proves the fd-release-alone consequence (Phase 30).

---

## §b — HARDEN-02: authenticated audit chain (`verify_chain`)

**Requirement:** `verify_chain` becomes forge-resistant against the in-host DB-writer. **Lands Phase 28.**

### Current-code anchor (re-verified at authoring)

- Schema, `crates/brokerd/src/audit.rs`: `events(id, parent_id, session_id, event_type, actor,
  payload, taint, parent_hash, hash)` — `hash`/`parent_hash` are plain `TEXT`, no secret-tied
  constraint (`STRICT` enforces column type only, not content).
- `compute_event_hash`: `SHA256(parent_hash.unwrap_or("") || id || session_id || event_type ||
  payload || taint)` — an **UNKEYED** hash. Anyone with read access recomputes it; anyone with WRITE
  access edits a row and recomputes a self-consistent `hash`/`parent_hash` for every descendant.
- `append_event`: INSERT-only (no `UPDATE`/`DELETE` on `events` outside `#[cfg(test)]` tampering helpers).
- `verify_chain` (`audit.rs:477-539`): a recursive CTE from `parent_id IS NULL`, walking forward,
  recomputing `compute_event_hash` per row. **It has no way to know how many events SHOULD exist.**
  A tail-truncation (`DELETE` the last N rows) or a restore-from-backup rollback makes the walk
  terminate at the now-shorter true leaf and return `true` (`found_any` needs only ≥1 row).
  **Confirmed: tail truncation is currently undetectable, full stop** — D-04's anchored/monotonic head
  is not decorative.
- **D-06 mutable field, precisely identified:** `blocked_literals` (side table keyed `(event_id, arg)`,
  `audit.rs:58`) is **deliberately kept OUT of the hashed `events.payload`** — only the digest
  `literal_sha256` is anchored; `redact_blocked_literal` is a real shipped `DELETE`.
- **Second mutable table (RESEARCH finding, not named by D-06): `pending_confirmations`**
  (`audit.rs:86`). Its `state` column is mutated in place via `transition_state`'s
  `UPDATE pending_confirmations SET state=?1 WHERE effect_id=?2 AND state='pending'`
  (`confirmation.rs:296`). It has **no hash/MAC column of its own** and is never inside the `events`
  chain — a DB-writer could flip `Confirmed`→`Pending` (defeating at-most-once) or delete the row, and
  `verify_chain` (scoped to `events`) would not detect it.
- `verify_chain` callers (exhaustive grep): (1) `confirmation.rs:599` inside `confirm()`, gating Step
  4.5a **in a SEPARATE, later OS process** (`caprun confirm`/`deny` are always fresh processes —
  `audit.rs:69-71` doc comment); (2) `cli/caprun/src/main.rs:343`, an end-of-run assertion in the SAME
  process. BOTH must stay true-on-untampered-chain.

### Mechanism (pinned)

**Keyed MAC (HMAC-SHA256) over the existing `compute_event_hash` input shape** — `sha2`/`hex` are
already `crates/brokerd/Cargo.toml` workspace deps; wrap, don't replace, the hash. The key is held by
the broker process, **OUTSIDE the confined worker's Landlock filesystem scope AND OUTSIDE the SQLite
file** (a bare DB-file writer, D-04's threat model, must not derive it from the DB).

**KEY-CUSTODY-ACROSS-PROCESSES (pinned — the single hardest question, RESEARCH §b / Open-Q2).**
`caprun` is **single-shot-per-session**: there is no persistent broker daemon. `confirm()`/`deny()`
run in a **separate, later OS process** that must verify the SAME chain the original run appended. A
per-process fresh key therefore **breaks `confirm()`'s `verify_chain` gate** (`confirmation.rs:599`)
— the exact call site that most needs the MAC. **The key MUST be a stable secret shared across the
`caprun` run and the later `confirm`/`deny` process.** Pinned concrete source: a **sibling
`<audit_path>.key` file outside the workspace root**, created with restrictive host permissions
(0600) at first `open_audit_db`, read by both `caprun` and `caprun confirm`/`deny`. It lives beside
the audit DB but is **never inside the workspace root** the confined worker's Landlock policy can reach,
and **never inside the DB file** the in-host DB-writer can read. Key generation uses a vetted
`getrandom`-backed RNG, never a custom PRNG. This custody model is NOT named by CONTEXT.md D-04 and is
pinned here explicitly rather than left as "the broker holds the key."

**Anchored/monotonic head (pinned, D-04).** A dedicated single-row table
`chain_anchor(session_id, head_event_id, head_hash, event_count)`, itself MAC'd with the same broker
key, updated **atomically with every `append_event`** (same lock/transaction discipline as
`mint_from_read`'s two-write atomicity). `verify_chain`'s new contract: recompute the walk AND assert
the computed leaf matches the anchor's `head_hash`/`event_count`. This turns tail-truncation from
invisible into **detected** (an attacker deleting the tail cannot re-MAC the anchor without the key).

**D-06 ruling (pinned).** Because `blocked_literals` already lives **outside** the hash (only
`literal_sha256` is anchored), a whole-row MAC over exactly the fields `compute_event_hash` covers
today **already satisfies D-06** — **no salted commitment is needed for `blocked_literals`.** State
this explicitly; the tamper-evidence-vs-redactability split the existing architecture made is
preserved by construction.

**`pending_confirmations` ruling (pinned — fold, not silently assume covered).** The second mutable
table is **folded into the MAC scheme**: MAC `state` (and `combined_digest`, already load-bearing for
CONFIRM-03) per row with the SAME broker key, re-checked at `confirm()`/`deny()` entry alongside the
chain-verify gate. This closes the flip-back/delete gap and is a natural sibling to §c's CAS work
(same table, adjacent surface), scoped to Phase 28 per the roadmap. (The alternative — naming it a
second Accepted Residual Risk — is explicitly **rejected**; folding is recommended and pinned.)

**Migration (pinned).** A pre-existing unauthenticated `audit.db` MUST fail closed on first
post-upgrade open (never silently trusted as "already authenticated"), using the same idempotent,
`PRAGMA table_info`-gated, re-run-safe discipline `migrate_pending_confirmations_schema` (`audit.rs:120`)
already demonstrates. An old DB with no anchor row is untrusted until re-anchored.

### Fail-closed default

Any of: chain-walk mismatch, computed-leaf ≠ anchor, `event_count` mismatch, missing/absent anchor
row, MAC-verify failure on a `pending_confirmations` row → `verify_chain`/the confirm gate returns
**false / DigestMismatch** and `confirm()` refuses to trust the read-back block. Absence of the key
file is fail-closed (cannot verify → refuse), never fail-open.

### Risk / false-positive surface

- **Key-custody is the load-bearing unknown (RESEARCH Assumption A2, highest stakes).** Get it wrong
  and either (a) `confirm()` can never verify an earlier process's chain (false-positive DigestMismatch
  on every legitimate confirm — breaks the shipped `s9_live_block.rs` confirm-path tests), or (b) the
  key lands somewhere a bare DB-writer or the confined worker CAN read, silently voiding D-04. This is
  the #1 DESIGN-12 review focus.
- **Anchor write must be atomic** with the `events` append it anchors — a non-atomic anchor reopens the
  truncation gap (delete anchor row + tail together).
- **Regression suite:** all `#[cfg(test)]` tamper-simulation tests in `audit.rs`/`confirmation.rs` (the
  self-consistent-edit and digest-mismatch-retry tests) must re-verify true-on-untampered under the new
  MAC'd `verify_chain` — the "no false positives on an untampered chain" criterion (Phase 30).

### Blast radius (Phase 28)

`audit.rs`: `compute_event_hash` signature (needs the key); `append_event` call-site; new `chain_anchor`
table in the schema DDL + a migration fn mirroring `migrate_pending_confirmations_schema`; `verify_chain`
rewritten to check the anchor. `confirmation.rs`: `confirm()` Step 4.5a (new key-loading dependency);
`transition_state` if `pending_confirmations` gets MAC'd. `cli/caprun/src/main.rs`: key generation/load
at startup near `open_audit_db`; `run_confirm_or_deny` needs the SAME key-load logic (shared helper).

---

## §c — HARDEN-03: Allowed-path replay CAS

**Requirement:** the trusted (Allowed) `email.send` path is replay-safe. **Lands Phase 29.**

### Current-code anchor (re-verified at authoring)

- The Allowed `email.send` dispatch lives in `evaluate_plan_node_and_record`
  (`crates/brokerd/src/server.rs`), the `matches!(decision, Allowed) && plan_node.sink.0 == "email.send"`
  block (`server.rs:792` onward). **No `PendingConfirmation` row, no CAS, no idempotency check of any
  kind exists here today.** It resolves the args, appends an OPAQUE `email_send_attempted` Event BEFORE
  any SMTP connection opens (`server.rs:820-846`, "MAJOR-4"), then invokes the SMTP send. The code's own
  comment (`server.rs:849`) names the residual: a replayed `SubmitPlanNode` mints a fresh `effect_id` and
  would send again (N submissions ⇒ N emails).
- **`effect_id` is minted fresh, per-call, at the TOP of `evaluate_plan_node_and_record`**
  (`let effect_id = Uuid::new_v4();`, `server.rs:562`) — BEFORE the executor runs. **This is the
  load-bearing fact:** an `effect_id`-keyed CAS (mirroring the confirm path's `PendingConfirmation`
  primary key) would do **NOTHING** against replay — every resubmission of the identical
  `SubmitPlanNode` gets a brand-new `effect_id` and sails through any `effect_id`-keyed uniqueness check.
- **The mirror-target already exists and is proven** on the CONFIRM path's `email.send` arm
  (`confirmation.rs`, "SEND-01"): `tx = conn.transaction()`; `affected = transition_state(&tx, effect_id,
  Confirmed)` (`confirmation.rs:296`, a single `UPDATE ... WHERE effect_id=?2 AND state='pending'` — the
  `affected==0` return IS the CAS); append `email_send_attempted` inside `tx`; `tx.commit()`; THEN open
  SMTP. But it is keyed on `effect_id`, which is stable **only** on the confirm path because `effect_id`
  is looked up from a persisted `PendingConfirmation` row created once at Block-time — never re-minted.
  The Allowed path has no such persisted anchor; it mints fresh every call. A literal copy-paste does
  not work without first fixing the key derivation.
- `PlanNode { sink: SinkId, args: Vec<PlanArg { name, value_id }> }` carries no `effect_id`, nonce, or
  content-hash field, and its shape is locked (CLAUDE.md: never introduce a raw `EffectRequest`). Any
  idempotency key MUST be **derived**, not carried as a new `PlanNode` field.

### Mechanism (pinned)

**Derive the idempotency key from the RESOLVED plan-node content, not from `effect_id` and not from
`plan_node` alone.** Two separate `SubmitPlanNode` messages carrying the exact same `value_id`s for the
exact same `sink` are a textbook replay. Pinned key:

> `SHA256( sink.0 || sorted( (arg_name, value_id) pairs ) )`

computed at the top of the `email.send` Allowed block, mirroring `confirmation.rs`'s `combined_digest`
shape (sorted-by-name, hash-of-pairs; `confirmation.rs:64`). **`value_id`-scope is the correct D-08
per-plan-node scope** (not resolved-literal scope): D-08 explicitly scopes CAS to per-plan-node, so
keying on the broker-scoped `value_id` handles is correct, not merely convenient — it deliberately does
NOT try to catch a worker that mints a NEW derivation resolving to the identical literal (that is
D-08's out-of-scope case).

**New table (pinned shape):**

> `sent_plan_nodes(idempotency_key TEXT PRIMARY KEY, effect_id TEXT NOT NULL, session_id TEXT NOT NULL, sent_at TEXT NOT NULL)`

**CAS idiom (pinned):** `INSERT INTO sent_plan_nodes (...)` inside the SAME transaction as the
`email_send_attempted` append; a **PRIMARY-KEY-constraint violation on replay IS the CAS**
(constraint-violation-as-signal — either `INSERT OR IGNORE` + check `rows_affected()==0`, or catch the
constraint error; pick one explicitly in Phase 29). This is a **different SQL idiom** from
`transition_state`'s `UPDATE ... WHERE` — chosen because the Allowed path has no pre-existing row to
`UPDATE` against; document the choice.

**Commit boundary (pinned):** CAS + `email_send_attempted` append commit **BEFORE any SMTP socket
opens**, exactly mirroring SEND-01 ("AFTER commit — the CAS + attempt are now durable together, or
neither is; only now does an SMTP connection ever open"). Copy that discipline into `server.rs:792-866`.

### Fail-closed default

A replay whose `INSERT` violates the PK constraint → the send is **suppressed** (no second SMTP
connection); at-most-once is preserved because the CAS and the attempt-append are one atomic unit. A
concurrent double-submit for the SAME plan node serializes on the `INSERT`/constraint under WAL (already
enabled) — state the isolation assumption explicitly in Phase 29.

### D-08 scope caveat (restated UNSOFTENED)

CAS buys **at-most-once PER PLAN NODE, not bounded sends per session.** A statically-compromised worker
that mints fresh, distinct `value_id`s (e.g. re-calls `mint_from_intent`/`mint_from_derivation` for the
"same" recipient) gets a DIFFERENT idempotency key and sends N times. No plan-layer dedup defeats that.
The **effects-budget / per-session rate-limit** is filed as named future defense-in-depth, **out of v1.6
scope** (§ Accepted Residual Risks).

### Blast radius (Phase 29)

`server.rs`: the `email.send` Allowed block (`:792-866`) — new key computation + CAS-guarded transaction
wrapping the existing `email_send_attempted` append. `audit.rs`: new `sent_plan_nodes` table + migration
fn (mirroring `migrate_pending_confirmations_schema`). `confirmation.rs`: no code change — its SEND-01
pattern is the cited template; a future reader diffing the two should see intentionally-parallel
structure. Tests: a new negative test — submit the SAME `SubmitPlanNode` twice on a trusted path, assert
exactly one Mailpit delivery (Phase 30, `s9_control_ab`-style A/B).

---

## §d — HARDEN-04: compile-out the forced-Active mint

**Requirement:** the forced-`Active` `CreateSession` mint is physically absent from a default release
build. **Lands Phase 27.**

### Current-code anchor (re-verified at authoring)

- `BrokerRequest::CreateSession { intent_id }` (`crates/brokerd/src/server.rs:904-994`) is gated by
  `if !matches!(std::env::var("CAPRUN_ENABLE_IPC_CREATE_SESSION").as_deref(), Ok("1")) { ...Error...;
  return }` (`server.rs:918-932`, exact-string-`"1"` match — the "F3 hardening" guard against an
  inherited empty-string env var). On success it mints a fresh session `SeedProvenance::TrustedArg`
  (forced `Active`), independent of the connection's own session.
- **`crates/brokerd/Cargo.toml` has NO `[features]` section today** (confirmed) — D-09's Cargo feature
  is a genuinely new addition to this crate.
- **A live, shipped, exact-shape precedent exists in a sibling crate:** `crates/executor/Cargo.toml`
  (`[features]` at `:22`, `test-fixtures = []` at `:28`, self dev-dependency
  `executor = { path = ".", features = ["test-fixtures"] }` at `:36`) paired with the dual-arm gating
  idiom used in `crates/executor/src/sink_schema.rs` (`#[cfg(any(test, feature = "test-fixtures"))]` at
  `:75`/`:93`, `#[cfg(not(any(test, feature = "test-fixtures")))]` at `:98`). This is exactly the
  `test-fixtures` feature name D-09 recommends and solves the known complication: `#[cfg(test)]` is NOT
  set when `brokerd` compiles as a dependency of an integration-test binary (`crates/brokerd/tests/uds_ipc.rs`,
  `.../planner_capability_split.rs` link `brokerd` as an ordinary non-`--cfg test` dep) — which is why
  the runtime flag was chosen originally, and which the feature-flag approach closes.
- Only `crates/brokerd/tests/uds_ipc.rs` actually EXERCISES the arm (three tests under a
  `CREATE_SESSION_ENV_LOCK` mutex); `planner_capability_split.rs` only references the `CreateSession`
  VARIANT (to test `ConnectionRole::Planner::permits()` denial) — unaffected either way.

### Mechanism (pinned)

Add `[features] test-fixtures = []` to `crates/brokerd/Cargo.toml` plus a self dev-dependency
`brokerd = { path = ".", features = ["test-fixtures"] }` — copy the `crates/executor/Cargo.toml` shape
verbatim (surgical; do not restructure the dependency graph). Gate the `CreateSession` forced-Active
body on `#[cfg(any(test, feature = "test-fixtures"))]`, with a **`#[cfg(not(any(test, feature =
"test-fixtures")))]` sibling arm that returns the SAME `Error` response the runtime flag returns today**
— identical wire behavior (an IPC caller still gets a clean `Error`, not a drop/panic), but the
mint-`Active` code path is **physically absent** from a release build (the `sink_schema.rs:98`
`test_schema_for` sibling is the precedent for this "no behavior change on the negative path, only
physical presence changes" discipline).

### D-10 own negative gate (pinned — a genuinely NEW discipline, no codebase precedent)

Because Cargo unifies features, a plain `cargo test` builds the lib WITH the feature — so the mitigation
needs its **own** negative gate or it verifies nothing. **Pinned primary (option c): a featureless-build
BEHAVIORAL negative test** — built WITHOUT `test-fixtures`, hitting `CreateSession` over a real socket,
asserting it ALWAYS returns the fail-closed `Error` (**no env var to set, no feature to enable** — no
possible opt-in). This proves behavioral absence (the only externally-observable thing that matters) and
is buildable with existing infrastructure (a variant of `uds_ipc.rs`'s
`create_session_over_ipc_denied_by_default_when_flag_unset`, in a config with no opt-in). **Binary
symbol-inspection (option a — `nm`/`strings`/`objdump` for a unique symbol) is optional
defense-in-depth only** — it bit-rots across Rust/LLVM versions and release optimization can
inline/strip either way (false negatives). Do NOT make (a) the primary gate.

**Phase 30 featureless-binary confirmation (pinned).** `scripts/mailpit-verify.sh` runs
`cargo test --workspace` by default, which DOES pull in dev-dependency features — so it is NOT
automatically the featureless build. Phase 30's live proof MUST run a genuinely **featureless release
binary** (`cargo build --workspace --release`, no test targets, no dev-deps) as the artifact actually
exercised, distinct from the `cargo test` run that exercises the feature-gated fixtures.

### Fail-closed default

In a featureless (release) build the `CreateSession` mint arm does not exist; the `#[cfg(not(...))]`
sibling returns `Error` unconditionally — there is no runtime input, env var, or feature that re-enables
the forced-`Active` mint. Absence is the fail-closed state.

### Risk / false-positive surface

- **Silently losing test coverage is the exact failure D-10 exists to prevent.** If Phase 27 gates the
  arm but forgets to propagate `test-fixtures` into `uds_ipc.rs`'s effective build, those three tests
  silently hit the new fail-closed arm — and
  `create_session_over_ipc_denied_by_default_when_flag_unset` would "pass" for the WRONG reason (arm
  gone, not flag-gated). RESEARCH Assumption A4: Cargo's self-feature-unification (proven for `executor`)
  reaching `brokerd/tests/*` **MUST be verified empirically in Phase 27** (actually run the tests), not
  assumed by inspection — `brokerd`'s `[dev-dependencies]` graph is richer than `executor`'s.
- `cargo test --workspace` unifies features workspace-wide; confirm no other member requests `brokerd`
  with `features=[...]` that leaks `test-fixtures` into a build that shouldn't have it (`cli/caprun`
  depends on `brokerd` with no explicit features today — re-confirm once `brokerd`'s manifest changes).

### Blast radius (Phase 27)

`crates/brokerd/Cargo.toml`: new `[features]` block + self dev-dependency. `server.rs`: `CreateSession`
arm split into two `#[cfg(...)]` siblings. `crates/brokerd/tests/uds_ipc.rs` (and
`planner_capability_split.rs`): dev-dependency feature propagation (verify, don't assume). New: a
featureless-build behavioral negative test (D-10's gate). `scripts/mailpit-verify.sh` / Phase 30: an
explicit no-feature `--release` build step distinct from the default `cargo test`.

---

## §e — HARDEN-05: `file.create` `contents` slot

**Requirement:** `file.create`'s `contents` gets I2 / slot-type treatment without regressing the only
live `file.create` flow. **Lands Phase 29.**

### Current-code anchor (re-verified at authoring)

- `expected_role(sink, arg_name)` (`crates/executor/src/sink_sensitivity.rs:147`): `"contents" => None`
  at `:157` (unconstrained). `is_content_sensitive` (`:102`) matches ONLY `"email.send" =>
  EMAIL_SEND_CONTENT_SENSITIVE.contains(&arg_name)` (`:104`) — **`file.create`'s `contents` is not in
  `is_content_sensitive`'s match arms at all today.** `is_routing_sensitive` (`:86`) excludes `contents`
  (`FILE_CREATE_ROUTING_SENSITIVE = &["path"]`, `:66`).
- The existing unit assertion `file_create_contents_is_unconstrained` (`:313-317`) asserts
  `expected_role(&file_create(), "contents") == None` — this assertion **INVERTS** under the change.
- **The single most load-bearing finding: `contents`' ONLY production value today is the reused
  `path`-role `intent_value_id`.** Traced end-to-end:
  - `cli/caprun/src/worker.rs` (`CreateFileFromReport` arm) reports ONLY `WorkerClaim::RelativePath`
    claims — **no `contents`-shaped claim extraction exists anywhere** for `file.create`; no doc-derived
    file body is ever minted.
  - `crates/brokerd/src/server.rs:1312`: `CaprunIntent::CreateFileFromReport { path } => (path.clone(),
    "path", None, None)` — `primary_role` is hardcoded `"path"`; it is the SOLE trusted literal this
    intent mints, then minted with `origin_role: Some("path")` (`server.rs:1337`).
  - `cli/caprun/src/planner.rs:208`: `PlanArg { name: "contents".into(), value_id: intent_value_id }` —
    `intent_value_id` (role `"path"`) is placed into BOTH the `path` slot AND the `contents` slot of the
    same plan node (comment at `:205-207` acknowledges the placeholder reuse). This is the LIVE, tested
    production behavior (`cli/caprun/tests/s9_live_block.rs::s9_live_file_create_clean_allow`).

### Mechanism (pinned — TWO edits, RESEARCH CORRECTION (i))

**(a)** Add a `"file.create" => FILE_CREATE_CONTENT_SENSITIVE.contains(&arg_name)` arm to
`is_content_sensitive`, with `FILE_CREATE_CONTENT_SENSITIVE = &["contents"]` (new const, mirroring
`EMAIL_SEND_CONTENT_SENSITIVE`'s shape). Scoped to **`&["contents"]` ONLY** so `path` never becomes
content-sensitive (it remains routing-sensitive). This is what actually wires a FUTURE tainted `contents`
value into the I2 collect-then-Block path.

**(b)** Change `expected_role`'s `:157` entry from `"contents" => None` to:

> `"contents" => Some(&["path"])`

**This exact value is the load-bearing pin.** Justification is the end-to-end trace above: today's
ENTIRE `contents` vocabulary is the reused trusted `"path"`-role literal; there is no
`"contents"`/`"file_body"` role-producing mint site yet. **Any list NOT including `"path"` hard-`Deny`s
(`SlotTypeMismatch`) the ONLY existing, currently-passing `file.create` clean-allow flow**
(`s9_live_file_create_clean_allow`, ROADMAP SC-3 regression canary). Doing only (b) without (a) gives
`contents` a role check but does NOT route a tainted value into I2; doing only (a) without (b) leaves the
slot role-unconstrained — both edits are required.

**Not dead code, but a present no-op on the live path.** Because `contents` currently only ever carries
a `UserTrusted`/`"path"`-role value, (a) fires on nothing today — but the moment a real
content-extraction pipeline mints a doc-derived `contents` claim (D-12's deferred future), this wiring
makes I2 fire on it. State this honestly (mirrors `DESIGN-session-trust-state.md` §6's
"currently-unreachable, not yet tested against a live gap" framing) — do not imply it is exercised
end-to-end today.

### Fail-closed default

`Some(&["path"])` preserves the v1.5 `None`-vs-`Some(&[])` contract: at a role-checked slot, a value
with `None` role or a role ∉ `["path"]` is a `Denied`, never a silent pass to `Allowed`. `Some(&[])`
must never be constructed (a zero-valid-role slot is a design bug, not a runtime state) — Phase 29 MUST
NOT implement the lookup as `.unwrap_or(&[])`.

### Risk / false-positive surface

- **The single largest false-positive risk is an incomplete role list breaking the only live flow.** A
  Phase-29 implementer reasoning "mirror `body`'s `["body","doc_fragment"]`" without re-tracing
  `planner.rs:208`'s handle reuse would pick `Some(&["contents"])` or `Some(&["file_body"])` (roles that
  do NOT exist in the current mint vocabulary) and immediately regress `s9_live_file_create_clean_allow`.
  This is a locked, non-negotiable table entry — not "Claude's Discretion."
- **Accidentally widening `path` to content-sensitive.** A careless copy of the email pattern
  (`"file.create" => true` unconditionally, rather than `.contains(&arg_name)`) would make `path`
  content-sensitive — a real regression. `FILE_CREATE_CONTENT_SENSITIVE` MUST be `&["contents"]` only;
  the existing `file_create_contents_not_routing_sensitive` / `file_create_path_is_routing_sensitive`
  test pair must stay green.
- **Weaker-than-it-sounds semantics (honest).** `contents`'s role check for now only verifies "this is A
  trusted CLI-supplied literal of some kind," not "this is trusted FILE CONTENT specifically" — because
  its only value's role genuinely is `"path"`. Record this in Accepted Residual Risks (mirrors D-12's
  own honesty framing); it is NOT a T2 violation (the value's role genuinely IS `"path"`, now an
  explicitly-accepted role for the slot).

### Blast radius (Phase 29)

`crates/executor/src/sink_sensitivity.rs`: `is_content_sensitive` (`:102`) — new match arm + new
`FILE_CREATE_CONTENT_SENSITIVE` const; `expected_role` (`:157`) — `"contents" => None` becomes
`"contents" => Some(&["path"])`. The `:313-317` unit assertion INVERTS and must be updated **deliberately**
in Phase 29, not left stale. `s9_live_file_create_clean_allow` is the regression canary (re-run, must not
`Deny`). No `PlanNode`/schema changes (`sink_schema.rs`'s `required: &["path","contents"]` unaffected).

---

## §f — Cross-Cutting Rulings (X-01, X-02, X-03) + the X-04 ruling

Three questions cut across the five residuals; each gets **ONE uniform rule**. A fourth (X-04) is a
NEW code-traced finding named by no locked decision — it gets its own dedicated ruling subsection so
the DESIGN-12 reviewer has a clean target.

### X-01 — label continuity (broker-written files)

**Current-code anchor.** No provenance/taint label of any kind is stamped on a broker-WRITTEN file
today. `invoke_file_create` (`crates/brokerd/src/sinks/file_create.rs`, the sole `file.create` writer,
`openat2(O_CREAT|O_EXCL|O_WRONLY, RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)`) writes bytes with no xattr,
sidecar, or DB row recording "path P was written by session X with taint Y." A write→re-read laundering
loop is theoretically possible but **not currently reachable** (§e: `contents` only ever carries a
trusted `"path"`-role literal, so no tainted bytes reach disk via `file.create` yet), and no live flow
both writes tainted `contents` AND later `RequestFd`s that same path in the same session. It becomes
live risk only once §e's content-extraction pipeline (D-12's deferred work) lands.

**Ruling (ONE model answers both HARDEN-01 and HARDEN-05).** Reuse §a's `is_trusted_labeled` as the
single label-provenance model: **a path is "trusted-labeled" ONLY if it equals the CLI's designated
`<workspace-file>` argument (`workspace_rel`).** A broker-WRITTEN file (via `file.create`) is, by this
rule, **automatically NOT trusted-labeled** — it was never the CLI's designated document — so a later
`RequestFd` on it **fail-closed-demotes by default.** This gives X-01 a fail-closed answer for v1.6
**without new xattr/sidecar machinery.** Recorded mechanism: *"broker-written files carry no explicit
label; they are treated untrusted-by-default via the same `is_trusted_labeled` fail-closed rule."* The
write→re-read laundering loop (D-12) is thereby **closed for v1.6**, and the full output-file provenance
model is explicitly **deferred** (§ Accepted Residual Risks).

### X-02 — shared-store recovery authority (REFRAMED — RESEARCH CORRECTION (iv))

**Framing correction.** CONTEXT.md's X-02 text ("a DB-writer can flip `SessionStatus` ... the change
goes live at broker restart") implies a persistent broker daemon that restarts mid-session. **That does
not describe this architecture.** `run_broker_server` is spawned **fresh, once, per `caprun` process
invocation** (`main.rs:238`) and lives only for that session's lifetime; `initial_session_status` is
seeded from `create_session`'s in-process result, **never re-read from the `sessions` table after
creation.** The REAL "restart" actor is the **`caprun confirm` / `caprun deny` / `caprun review`
process** — a fresh OS process opening `audit.db` fresh (`audit.rs:69-71` doc comment:
`pending_confirmations` "is the ONLY thing that survives to resume from").

**Ruling (ONE rule, applied uniformly to BOTH the `events` chain AND `pending_confirmations`).** On
**every** `confirm`/`deny` process start, all recovered security state — the `sessions.status` /
`events` chain (via HARDEN-02's anchor + MAC) AND `PendingConfirmation.state` / `resolved_args` /
`combined_digest` (which has **no MAC of its own today**, §b) — is **either MAC-re-verified (ties to
HARDEN-02) or fail-closed re-derived.** **Pinned choice: MAC-re-verified**, because §b already folds
`pending_confirmations` into the same broker-key MAC scheme; a failed verification is fail-closed
(refuse to resume the confirm/deny, never silently trust the disk row). This closes the DB-writer
flip-back/delete gap uniformly across both tables rather than only the `events` table CONTEXT.md's text
centered on.

### X-03 — TOCTOU / atomic ordering

**Current-code anchor.** The submit-side ordering already follows **authorize-before-effect**:
`executor::submit_plan_node` resolves its decision synchronously; the resulting Event is durably
appended **under lock** (`server.rs:709-751`) BEFORE any sink (`file.create`/`email.send`) is invoked
(`server.rs:761-866`). This is already the "status transition commits before effect dispatch" discipline.

**Ruling (ONE uniform atomicity rule).** **The status transition commits BEFORE effect dispatch** —
the same discipline as demote-before-fd-release in D-01 (§a) and CAS-before-SMTP in §c. Every new
mechanism in v1.6 inherits it: §a demotes-then-releases-fd; §b anchors atomically with the append; §c
commits the CAS + attempt before opening SMTP. The genuine remaining gap is **not** a within-`SubmitPlanNode`
race — it is the cross-connection staleness ruled on in X-04 below.

### X-04 RULING — Planner-connection `session_status` staleness (the NEW finding)

**This is the single most consequential grounding of the research; it is ruled on here explicitly, not
buried in §a prose.**

**Current-code anchor (re-verified at authoring).** Every connection `run_broker_server` accepts —
the Worker connection AND every subsequent Planner connection — seeds its own per-connection
`session_status` local from the SAME `initial_session_status` **function parameter**, cloned fresh per
connection: Worker at `server.rs:202` (`let initial_status = initial_session_status.clone();`), Planner
at `server.rs:231` (same clone), threaded into `classify_second_connection` / `handle_connection` with
`ConnectionRole::Planner`. `initial_session_status` is `run_broker_server`'s own parameter
(`server.rs:149`), set ONCE by its caller from the in-process `create_session()` result at
session-creation time — **never re-read from the `sessions` table when a second (Planner) connection is
accepted.** The ONLY post-creation mutation of `session_status` is `ReportClaims`'s
`*session_status = SessionStatus::Draft` (`server.rs:1131`), which mutates the **Worker connection's OWN
`&mut SessionStatus` local** — a value **NEVER visible to any other connection's task** (no
`Arc<Mutex<SessionStatus>>`, no shared cell; the signature is a plain owned-per-task `&mut`).

**Concrete consequence.** In the live v1.4/v1.5 adversarial-planner architecture the normal order is:
(1) worker connects, (2) worker `RequestFd`s + reads a hostile document, (3) worker `ReportClaims`
demotes the session to `Draft` (in the DB and in the worker connection's own local), (4) the planner
sidecar is engaged and a Planner-role connection is accepted, (5) the planner submits `SubmitPlanNode`.
**At step 4 the Planner connection's `session_status` local is seeded from `initial_session_status` —
the ORIGINAL `Active` value — NOT the `Draft` the worker demoted to at step 3.** Step 0.5 of
`submit_plan_node` (the I0 class-level `Draft` + `CommitIrreversible` deny) therefore evaluates against a
**STALE `Active` status** for every plan node the Planner submits, silently bypassing the class-level
deny. It has never been caught because every shipped adversarial-planner test routes its tainted literal
through a routing/content-sensitive arg, so I2's per-arg Block fires FIRST and Step 0.5 is never reached
(`DESIGN-session-trust-state.md` §11 condition 4's documented precedence) — the bypass is real but
un-exercised.

**RULING — fold into HARDEN-01's Phase-27 scope (option a).** This staleness is folded into HARDEN-01
as a **required fix**, not accepted as a residual. Rationale: (i) I0's whole point is defending the
"clean-looking values from an untrusted context" case this bug reopens **specifically on the Planner
path** — accepting it would be hard to justify; (ii) X-03 already commits to "status transition commits
before effect dispatch" as the uniform atomicity rule, and a per-connection stale COPY of
`session_status` is a species of the same problem with a **wider blast radius** (any point later in the
run, not a narrow race window); (iii) the fix is small and has a live precedent.

**Pinned fix.** Make `session_status` a **shared `Arc<Mutex<SessionStatus>>`** across the WHOLE
`run_broker_server` invocation, **re-read at the top of every `dispatch_request`** rather than seeded
once per connection — mirroring how `planner_slot_occupied: Arc<AtomicBool>` is ALREADY a shared handle
for exactly the same cross-task-visibility reason (`server.rs:185`). A Worker-connection demotion then
becomes immediately visible to the Planner connection's Step 0.5.

**Requirements bookkeeping.** This is ruled **in-scope-by-extension of HARDEN-01** ("fd release itself
carries the I1 draft-only consequence" — the bug is specifically about `session_status` not being
consistently visible across connections at the moment ANY deny decision is evaluated), so **no new
HARDEN-0X requirement id is minted.** If the owner prefers a distinct id for traceability, Phase 27's
planning may add one; the design ruling does not depend on it. A Planner-connection-after-demotion
negative test (currently unassigned to any phase) becomes a Phase-27 negative test under this ruling.

---

## §g — Adversarial-Review Preemption

A fresh non-self reviewer (DESIGN-12's gate) will probe these; each is answerable by **tracing the
cited code**, not by trusting this doc. Mirrors `DESIGN-slot-type-binding.md` §8.

1. **§a — can a demotion be skipped by a silent worker?** No: the demotion moves to `RequestFd`'s
   broker-side entry (symmetric with `*fd_requested = true`), fail-closed on any non-`workspace_rel`
   path, committed BEFORE the fd is released. A worker that never sends `ReportClaims` still gets
   demoted the moment it reads an untrusted path. Reviewer traces the `RequestFd` arm and confirms
   `session_status` is now mutated there.
2. **§b — can a bare DB-writer forge a chain, and does key custody leak to the worker? (RESEARCH
   Assumption A2, highest stakes.)** No forge without the key: the MAC key lives in
   `<audit_path>.key` outside the workspace root (worker Landlock never reaches it) and outside the DB
   file (the in-host DB-writer never reads it); tail-truncation is caught by the MAC'd `chain_anchor`.
   The reviewer MUST adversarially probe the key-custody path (can the confined worker's Landlock policy
   ever include the key file? does `confirm`/`deny` obtain the SAME key?) — this is THE review focus.
3. **§c — does an `effect_id`-keyed CAS actually stop replay?** No — and the doc says so loudly:
   `effect_id` is minted fresh per `SubmitPlanNode` (`server.rs:562`), so the CAS MUST key on
   `SHA256(sink || sorted(arg_name, value_id))`, not `effect_id`. Reviewer confirms the key derivation
   is content-derived, and that CAS+append commit before any SMTP socket opens.
4. **§d — does a plain `cargo test` silently build WITH the feature, defeating the gate?** Yes for
   `cargo test` (Cargo unifies features) — which is exactly why the gate is a **featureless-build
   behavioral negative test** (option c) plus a Phase-30 `cargo build --workspace --release` featureless
   binary, NOT a `cargo test` assertion. Reviewer confirms the negative gate has no opt-in path.
5. **§e — does the `contents` role list break the only live flow?** No, and only because it is pinned to
   `Some(&["path"])`: the sole production `contents` value is the reused `"path"`-role handle
   (`planner.rs:208` → `server.rs:1312`/`:1337`). Any list omitting `"path"` regresses
   `s9_live_file_create_clean_allow`. Reviewer traces worker→server→planner and confirms `"path"` is the
   only live role.
6. **§f / X-04 — is the staleness fix (or its acceptance) sound?** The fix is a shared
   `Arc<Mutex<SessionStatus>>` re-read per `dispatch_request`, mirroring `planner_slot_occupied`'s
   existing `Arc<AtomicBool>` (`server.rs:185`). Reviewer confirms the Planner connection no longer
   reads a stale per-connection snapshot at Step 0.5, and that the fix does not re-introduce a
   worker-asserted trust flip (the demotion source stays broker-only).

---

## §h — Accepted Residual Risks

Mirrors `DESIGN-slot-type-binding.md` §10. Each is a **documented, intentional** scope boundary, not an
accidental gap.

- **D-05 — full host/root compromise reading the broker MAC key.** Out of scope for v1.6; the threat
  ceiling is the in-host DB-writer (D-04). A root actor that reads `<audit_path>.key` can forge the
  chain. Candidate future defense: external out-of-store notarization (later milestone). The doc does
  NOT claim tamper-evidence beyond the in-host-DB-writer model.
- **D-08 — effects-budget beyond per-plan-node CAS.** §c's CAS buys at-most-once PER PLAN NODE; a
  statically-compromised worker minting N distinct plan nodes still sends N times. A per-session
  effects-budget / send rate-limit is named future defense-in-depth, out of v1.6 scope.
- **D-12 — full output-file provenance labeling.** Deferred; v1.6 uses §e's input-role treatment plus
  §f/X-01's `is_trusted_labeled` fail-closed continuity rule (broker-written files are untrusted-by-default).
  The write→re-read laundering loop is closed for v1.6 by that fail-closed default, not by real write-time
  labeling.
- **§e weaker-than-it-sounds semantics.** `contents`'s role check currently verifies only "a trusted
  CLI-supplied literal of some kind," not "trusted FILE CONTENT" — because its only value's role is
  `"path"`. Honest, documented; not a T2 violation.

(No X-04 residual is recorded, because X-04 is **ruled fold-not-accept** in §f — it is a required
Phase-27 fix, not an accepted residual.)

---

## §i — Phase 27–30 Implementation Map (informative — not part of the gate)

Anticipates the mechanical work the gate unblocks. Grounded, but each phase **re-verifies file:line** if
commits intervene. Mirrors `DESIGN-slot-type-binding.md` §9.

| Phase | Residuals | Primary files |
|---|---|---|
| **27** | HARDEN-01 (§a) + HARDEN-04 (§d) + X-04 fix (§f) | `crates/brokerd/src/server.rs` (RequestFd demotion, `run_broker_server`/`dispatch_request` signatures, shared `Arc<Mutex<SessionStatus>>`, `CreateSession` dual-`#[cfg]` arm), `crates/brokerd/Cargo.toml` (`[features] test-fixtures`), `quarantine.rs` (doc-comment fix), `cli/caprun/src/main.rs` (`workspace_rel` threading), `DESIGN-session-trust-state.md` (§2/§5 amendment) |
| **28** | HARDEN-02 (§b) | `crates/brokerd/src/audit.rs` (keyed MAC, `chain_anchor` table + migration, `verify_chain` rewrite), `confirmation.rs` (`pending_confirmations` MAC, `confirm()` key-load), `cli/caprun/src/main.rs` (key gen/load) |
| **29** | HARDEN-03 (§c) + HARDEN-05 (§e) | `crates/brokerd/src/server.rs` (`email.send` Allowed CAS), `audit.rs` (`sent_plan_nodes` table + migration), `crates/executor/src/sink_sensitivity.rs` (`is_content_sensitive` arm + `expected_role` `Some(&["path"])`) |
| **30** | HARDEN-06 (full regression + 5 negative tests) | `cli/caprun/tests/*`, `crates/brokerd/tests/*`, `scripts/mailpit-verify.sh` (featureless release build step) |

**Gate invariants each phase must not trip.** `check-invariants.sh` Gate 1 (no `EffectRequest` token
under `crates/` — keep §c's key a `(SinkId, args)`-derived hash, never a raw args-map-to-sink path);
Gate 3 (`mint_from_read(` / `mint_from_derivation(` / `.mint(` call-site tokens restricted to
`quarantine.rs` / `server.rs` / `value_store.rs` — §a's demotion reuses `update_session_status`, adds no
new mint call site). New enum variants (if any) rely on Rust's own exhaustive-match check + inline
blast-radius grep documented per phase (there is no scripted no-wildcard gate — state the grep result the
way `DESIGN-slot-type-binding.md` §5 did). Neither HARDEN-02 nor HARDEN-05 needs a new `DenyReason`
variant (HARDEN-02's forged-chain outcome reuses the existing `verify_chain==false` path; HARDEN-05
reuses the existing `SlotTypeMismatch`/I2 machinery).

---

## §j — Proof-plan Note (for Phase 30)

Standing close-gate disciplines pinned now so Phase 30 inherits them:

- **Use the BARE `scripts/mailpit-verify.sh` recipe** for all Linux verification (from Phase 16 onward,
  per CLAUDE.md — CONTROL-01 makes a benign run capable of a live SMTP send, so a Mailpit sidecar must be
  present even for "looks benign" tests). Scope a single test via `MAILPIT_VERIFY_CMD`.
- **Capture `$?` BEFORE any pipe.** A `script | tail` returns `tail`'s status (always 0); assert on the
  **PASSED sentinel + named test counts**, never on exit-0-through-a-pipe (this project's own standing
  `verification-exit-code-through-pipe` discipline).
- **Run a FEATURELESS release binary** (`cargo build --workspace --release`, no dev-dep feature
  unification) as the artifact actually exercised for the HARDEN-04 proof (§d) — distinct from the
  `cargo test` run that exercises the feature-gated fixtures.
- **One negative test per closed residual** (Phase 30 map): forged/truncated chain rejected by
  `verify_chain` (§b); replayed Allowed `email.send` delivers exactly once to Mailpit (§c); forced-Active
  `CreateSession` arm absent/fail-closed in the featureless release binary (§d); `RequestFd` on an
  untrusted path (no `ReportClaims`) demotes to `Draft` WHILE the CONTROL-01 clean path still sends (§a);
  tainted `contents` Blocks once a mint site exists, and the live `s9_live_file_create_clean_allow`
  still Allows (§e); Planner-connection `SubmitPlanNode` after a Worker demotion is denied at Step 0.5
  (§f/X-04).

---

## Acceptance Predicate — Done When

Phase 26's gate is cleared when ALL are true:

1. This doc pins, for **each of the five residuals** (§a–§e), a **mechanism + fail-closed default**
   grounded in a re-verified file:line anchor. **(DESIGN-11)**
2. This doc pins the **three cross-cutting rulings** (§f: X-01 label continuity, X-02 shared-store
   recovery authority reframed to the `confirm`/`deny` process + `pending_confirmations`, X-03 TOCTOU
   commit-before-dispatch) as ONE uniform rule each.
3. This doc **rules explicitly on X-04** (the NEW Planner-connection `session_status` staleness finding)
   — fold-into-HARDEN-01 (chosen) vs named-residual — in a dedicated subsection, not silently inherited.
4. This doc incorporates all **four RESEARCH corrections**: (i) `contents` expected-role `Some(&["path"])`;
   (ii) HARDEN-01's `workspace_rel` new-plumbing budget; (iii) the X-04 ruling; (iv) X-02 reframed to the
   `confirm`/`deny` process + `pending_confirmations`.
5. This doc carries an **Adversarial-Review-Preemption §** (§g) and an **Accepted Residual Risks §** (§h),
   mirroring `DESIGN-slot-type-binding.md`.
6. This doc has **cleared a fresh, non-self adversarial review** (traced against real code) with every
   finding resolved, recorded in `DESIGN-GATE-RECORD-v1.6.md`, and **no `crates/executor` /
   `crates/brokerd` / `crates/runtime-core` hardening code exists yet.** **(DESIGN-12 — satisfied by
   Plan 26-02.)**

---

## Amendments (post-review)

Round-tagged amendments from the fresh adversarial review (DESIGN-12, Plan 26-02) are folded into the
relevant §above, per `DESIGN-slot-type-binding.md`'s convention. See `DESIGN-GATE-RECORD-v1.6.md` for
the full review.

_(none yet — pending Plan 26-02)_
