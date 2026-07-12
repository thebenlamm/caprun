# Phase 28: Authenticated Audit Chain - Research

**Researched:** 2026-07-12
**Domain:** Rust TCB hardening — SQLite audit-chain authentication (keyed HMAC over an existing hash chain, cross-process key custody, atomic anchor maintenance)
**Confidence:** HIGH (every code anchor below re-verified against live source this session; the `hmac`/`getrandom` crate choices were empirically compile-tested against the real workspace dependency graph, not assumed from memory)

## Summary

Phase 28 implements HARDEN-02 exactly as pinned by `planning-docs/DESIGN-security-hardening.md` §b (post-F1-amendment): wrap the existing unkeyed `compute_event_hash` in HMAC-SHA256, add a MAC'd `chain_anchor` table updated atomically with every `append_event`, fold `pending_confirmations.state`/`combined_digest` into the same MAC scheme, and — per the Round-1 BLOCKER fix — add a broker-enforced fail-closed startup refusal when the audit DB or its `.key` sibling resolves beneath the workspace root. The mechanism is locked; this research's job is to re-verify every cited anchor against live source (all of which now reflects Phase 27's landed changes) and surface concrete implementation facts the DESIGN doc's §b prose did not spell out at file:line granularity.

Three findings materially change Phase 28's task list and were **not** visible from the DESIGN doc alone:

1. **The F1 fail-closed startup refusal, implemented literally, breaks 7 of the 8 existing live-integration test files today.** `s9_live_block.rs`, `e2e.rs`, `live_acceptance_tainted_session.rs`, `live_acceptance_v1_3.rs`, `live_acceptance_v1_4_composed.rs`, `llm_planner_live_accept.rs`, and `origin_seed_provenance.rs` all place `audit.db` as a **sibling of the workspace file inside the same directory** (`workspace_file = tmp.join("workspace.txt")`, `audit_db = tmp.join("audit.db")`) — since `workspace_root_dir = workspace_file.parent()`, this makes `audit.db` a **direct child of the workspace root**, i.e. exactly the F1 vulnerability the refusal is designed to reject. Only `cli/caprun/tests/confirm.rs` already keeps them as siblings under a common parent (`workspace = tmp.join("workspace")`, `db_path = tmp.join("audit.db")` — genuinely separate). **Phase 28 must migrate all 7 fixtures' directory layout before the F1 refusal lands, or every one of those tests will fail to even start the broker.**
2. **`append_event` has 19 non-test production call sites** across `server.rs` (5), `quarantine.rs` (4), `confirmation.rs` (4), `sinks/file_create.rs` (4), and `sinks/email_smtp.rs` (2) — nearly double Phase 27's `dispatch_request` fanout (11). Since HMAC needs the key at every hash computation, `compute_event_hash`/`append_event`'s signatures must grow a `key` parameter reaching all 19 sites (plus ~20 more in `#[cfg(test)]` modules). **Recommendation: fold the `chain_anchor` upsert into `append_event` itself** (single choke point) rather than requiring a second call at each of the 19 sites — `append_event` already returns the newly-computed hash and has `event.session_id`/`event.id` in scope, so it has everything needed to also write the anchor row under the same already-held lock.
3. **`deny()` never calls `verify_chain` today — only `confirm()` does.** The DESIGN doc's X-02 ruling says "on every confirm/deny process start, all recovered security state... is either MAC-re-verified or fail-closed re-derived," naming both verbs, but the current code (`confirmation.rs:753-785`) has zero chain-verify or MAC-check call in `deny()`. This is a genuine scope gap between the DESIGN doc's cross-cutting ruling and its own §b blast-radius note (which only names `confirmation.rs:599`, inside `confirm()`). Phase 28's plan must decide explicitly whether `deny()` gains the same gate (X-02 says it should) — this is not optional cleanup, it's a locked ruling the DESIGN doc itself pins.

**Primary recommendation:** Add `hmac = "0.12"` (workspace-pinned, verified to compile against the existing `sha2 = "0.10"` dependency) and `getrandom = "0.4"` (already transitively present via `uuid`'s `v4` feature — adding it directly costs zero new dependency resolution) to `crates/brokerd/Cargo.toml`. Thread an explicit `key: &[u8]` parameter through `compute_event_hash`/`append_event`/`verify_chain` (mirroring the codebase's existing plain-parameter style, e.g. `parent_hash: Option<&str>`) rather than a global/thread-local. Fold the `chain_anchor` upsert into `append_event`'s own body. Build a single `load_or_create_key(audit_path) -> (Vec<u8>, refusal-check)` helper shared by both `main()`'s run path and `run_confirm_or_deny`. **Sequence the test-fixture directory migration (finding #1) as an early, explicit task — not a discovery made via a wall of red tests after the refusal lands.**

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| HARDEN-02 | `verify_chain` becomes an authenticated-integrity check — an actor with `events`-table write access can no longer produce a chain `verify_chain` accepts; a bare `events`-table writer cannot derive the key/anchor; an untampered chain still verifies true, no false positives, existing confirm-path and live-acceptance callers unaffected | Anchor table below pins the exact `compute_event_hash`/`append_event`/`verify_chain` locations, the 19-site `append_event` fanout, the key-custody-across-processes call sites (`main.rs:172-174` and `main.rs:384`), the F1 broker-startup-refusal blast radius against the 7 vulnerable test fixtures, the `pending_confirmations` fold target (`transition_state`, `confirmation.rs:296`), and the `migrate_pending_confirmations_schema` idempotent-migration template to mirror for `chain_anchor`. |

</phase_requirements>

## Project Constraints (from CLAUDE.md)

- **TCB is Rust; I2/I1/I0 are hardcoded, never a swappable policy file.** The MAC key and the fail-closed startup refusal are plain Rust control flow — no config-file-driven key location, no runtime toggle to disable the refusal. `check-invariants.sh` Gate 1 (no `EffectRequest` token) and Gate 3 (mint-call-site restriction) still run; HARDEN-02 touches no `PlanNode`/`ExecutorDecision`/mint-site surface at all (confirmed: this phase is entirely `audit.rs`/`confirmation.rs`/`main.rs` plumbing plus one new Cargo dependency), so neither gate needs an exemption — re-run `./scripts/check-invariants.sh` after this phase's changes as a sanity check, but no new exemption is anticipated.
- **Linux-only security tests show "0 passed" on macOS by design.** All Phase 28 enforcement tests (forgery-rejection, tail-truncation, F1 startup refusal, cross-process confirm) must run via `bash scripts/mailpit-verify.sh` (per CLAUDE.md, ALL Linux verification goes through this recipe from Phase 16 onward — a benign run can trigger a live SMTP send). Do not use the bare `docker run rust:1` recipe.
- **Surgical changes only.** Every changed line traces to HARDEN-02's DESIGN-doc mechanism. Do not touch HARDEN-03 (`email.send` CAS, Phase 29), HARDEN-05 (`file.create` `contents`, Phase 29), or re-open HARDEN-01/HARDEN-04 (Phase 27, already landed and verified with zero drift this session).
- **check-invariants.sh runs before any code.** Re-run after every task.
- **No `--no-verify`, no swallowed exceptions, no downgraded errors.** A `verify_chain`/MAC failure must surface as `DigestMismatch`/refusal, never a silently-passed check or a swallowed `Result`.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Keyed event-hash computation (`compute_event_hash` → HMAC) | Broker (reference monitor) | — | The hash-chain's authenticity is a broker-owned invariant; the key must never be derivable by the confined worker (sandbox tier) or a bare filesystem DB-writer. |
| MAC key generation + custody | Broker (build/startup boundary) | CLI orchestrator (`main.rs`, the process that first opens the audit path) | The key is created once per audit-DB lifetime, at first `open_audit_db`, and must be readable by every LATER process (`caprun confirm`/`deny`) that touches the same DB — this is a CLI-process-boundary concern, not an in-broker-server concern (there is no persistent broker daemon in this architecture). |
| F1 fail-closed startup refusal (audit/key path vs. workspace root) | CLI orchestrator (`main.rs`, before spawning the broker/worker) | Broker (`run_broker_server` entry, defense-in-depth) | The check must run BEFORE the confined worker can ever connect and `RequestFd` the key file — in `caprun run`'s single-process-spawns-broker-task architecture, `main.rs` is the earliest point both the audit path and the workspace root are known together. |
| `chain_anchor` maintenance (atomic with `append_event`) | Broker (reference monitor) | — | Purely an audit-substrate invariant; no worker or sandbox involvement — mirrors `mint_from_read`'s existing two-write atomicity discipline (same-lock, not SQL-transaction). |
| `pending_confirmations` MAC fold (`state`/`combined_digest`) | Broker (reference monitor) | — | Closes the flip-back/delete gap on the SAME actor model (in-host DB-writer) HARDEN-02 defends against; not a new threat surface. |
| Confirm/deny cross-process key reload | CLI orchestrator (`run_confirm_or_deny`) | Broker (`confirmation::confirm`/`deny`, which consume the loaded key) | `caprun confirm`/`deny` are always separate, later OS processes (no persistent broker) — the CLI is the only tier that can locate and re-open the same key file both processes must share. |

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `hmac` | `0.12.1` (latest is `0.13.0`) | HMAC-SHA256 construction wrapping `sha2::Sha256` | `[VERIFIED: crates.io registry + gsd-tools package-legitimacy check, verdict OK]` — the canonical RustCrypto MAC crate (`github.com/RustCrypto/MACs`), same publishing org as the already-pinned `sha2`/`digest` ecosystem. **Pin `0.12.1`, not the newest `0.13.0`** — empirically verified this session (`cargo check -p brokerd` with a scratch `Hmac<Sha256>` usage) that `0.12.1` compiles cleanly against the workspace's existing `sha2 = "0.10"` pin; `0.13.0` targets a newer `digest` major version that may not match `sha2 0.10`'s trait version (not tested — `0.12.1` is the safe, verified choice given the existing pin). |
| `getrandom` | `0.4` (latest, matches the version already resolved transitively via `uuid`'s `v4` feature) | CSPRNG-backed key-material generation for the MAC key file | `[VERIFIED: crates.io registry + gsd-tools package-legitimacy check, verdict OK]` — `getrandom 0.4.3` is ALREADY in `Cargo.lock` (pulled in by `uuid = { version = "1.23.4", features = ["v4", "serde"] }`) — adding it as a direct `brokerd` dependency costs **zero new dependency resolution**, only makes an already-compiled crate directly `use`-able. Empirically verified this session: `getrandom::fill(&mut buf)` (the 0.4 API) compiles cleanly in a scratch `crates/brokerd/src/audit.rs` probe. |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `sha2` | `0.10` (already pinned, `crates/brokerd/Cargo.toml:23`) | Underlying hash for both the existing unkeyed `compute_event_hash` (being wrapped, not replaced) and the `hmac` crate's `Hmac<Sha256>` type parameter | No change — reused as-is. |
| `hex` | `0.4` (already pinned, `crates/brokerd/Cargo.toml:24`) | Hex-encoding the HMAC output, matching the existing `hex::encode(hasher.finalize())` pattern in `compute_event_hash` | No change — reused as-is. |

**Version verification:** `cargo search hmac` → `hmac = "0.13.0"` (latest) with `0.12.1` as the immediately-prior version; `cargo search getrandom` → `getrandom = "0.4.3"` (latest, and the exact version already in `Cargo.lock` via `uuid`). Both verified against the live crates.io index this session (2026-07-12), not from training-data memory. `cargo info hmac`/`cargo info getrandom` confirm: `hmac` license `MIT OR Apache-2.0`, repo `github.com/RustCrypto/MACs`; `getrandom` repo `github.com/rust-random/getrandom` — both canonical, well-known crates, not lookalikes.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `hmac` crate's `Hmac<Sha256>` | Hand-rolled HMAC built directly on `sha2::Sha256` (XOR pads + two hash calls per RFC 2104) | Explicitly rejected — CLAUDE.md's "never hand-roll cryptography" principle (mirrored from the security-domain protocol's V6 category) applies directly; the vetted `hmac` crate has constant-time-safe construction and is the RustCrypto-ecosystem-standard pairing for `sha2`. Hand-rolling HMAC is a textbook "don't hand-roll" case even though the algorithm is simple — subtle bugs (e.g. wrong pad length, missing outer-hash truncation-resistance) are a known historical vulnerability class. |
| `getrandom` direct dependency | Reuse `uuid::Uuid::new_v4()` bytes as key material | Rejected — `Uuid::new_v4()` produces 128 bits (16 bytes) with 6 bits consumed by version/variant fields (122 bits of actual entropy), less than the 256-bit (32-byte) key HMAC-SHA256 wants for full-strength security, and `Uuid`'s public API is not documented as a general-purpose CSPRNG source (it happens to use `getrandom` internally, but that's an implementation detail, not a contract). Call `getrandom::fill` directly for a full 32-byte key — matches the DESIGN doc's explicit "vetted `getrandom`-backed RNG, never a custom PRNG" pin precisely. |
| Threading `key: &[u8]` through 19 `append_event` call sites | A `thread_local!`/global static holding the key | Rejected — an implicit global secret in a multi-connection async broker is exactly the kind of "swappable/implicit side channel" CLAUDE.md's TCB discipline warns against; it also makes the key's provenance untestable (a unit test cannot easily construct two DIFFERENT keys side-by-side to prove key-dependence) and diverges from every other explicit-parameter pattern already in this codebase (`conn`, `parent_hash`, `session_id` are all explicit params, never globals). |

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `hmac` | crates.io | published 2016-10-06 (~10 yrs) | 8,308,040/week | `github.com/RustCrypto/MACs` | OK | Approved |
| `getrandom` | crates.io | published 2019-01-19 (~7 yrs) | 34,698,709/week | `github.com/rust-random/getrandom` | OK | Approved |

**Packages removed due to [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none.

Both packages were verified via `gsd-tools query package-legitimacy check --ecosystem crates hmac getrandom` (verdict `OK` for both, high weekly-download counts, canonical GitHub source repos, no `deprecated` flag, no `postinstall` script signal — not applicable to Cargo but checked per the seam's generic signal set) AND independently compile-tested against the live workspace dependency graph this session (`cargo check -p brokerd` with scratch usage of both APIs, then fully reverted — `git status --porcelain` confirmed clean before and after). This exceeds the `[VERIFIED]` bar: both discovery (via `cargo search`, an authoritative registry query, not WebSearch/training-data) and registry-legitimacy-check AND empirical compile verification were performed in this session.

## Re-Verified Anchor Table (the load-bearing research output)

Every anchor below was re-read from live source this session, post-Phase-27 (all of Phase 27's HARDEN-01/HARDEN-04/X-04 changes are already landed and visible in the current tree).

| DESIGN doc citation | Current live-source location | Status | Note for planner |
|---|---|---|---|
| `events` schema DDL | `crates/brokerd/src/audit.rs:33-43` | **CONFIRMED, unchanged** | `hash TEXT NOT NULL`, `parent_hash TEXT` — plain columns, no MAC/key binding yet. |
| `compute_event_hash` | `crates/brokerd/src/audit.rs:243-259` | **CONFIRMED, unchanged** | `SHA256(parent_hash.unwrap_or("") \|\| id \|\| session_id \|\| event_type \|\| payload \|\| taint)` — UNKEYED. This function's signature is where the `key: &[u8]` param must land; swap `Sha256::new()` for `<Hmac<Sha256> as Mac>::new_from_slice(key)` and `hasher.update(...)`/`hasher.finalize()` for the `Mac` trait's equivalents (verified API shape this session — see Code Examples). |
| `append_event` | `crates/brokerd/src/audit.rs:274-315` | **CONFIRMED, unchanged** | Calls `compute_event_hash` at `:290-297`, then a single `INSERT`. **This is the single function to extend with the `chain_anchor` upsert** (recommendation #2 above) — it already has `event.session_id`, the newly-minted `event.id`, and the just-computed `hash` in scope, and every one of its 19 production callers already holds the connection lock (confirmed by inspection of every call site below). |
| `verify_chain` | `crates/brokerd/src/audit.rs:477-539` | **CONFIRMED, unchanged** | Recursive CTE from `parent_id IS NULL`; `found_any` (≥1 row) is the ONLY existence check — confirmed a tail-`DELETE` still terminates at the shorter true leaf and returns `true` (re-verified by reading the loop: nothing queries an expected count or head). The NEW contract must additionally: (1) load the `chain_anchor` row for `session_id`, verify ITS MAC, and (2) assert the walk's final `(id, hash)` and total row-count match the anchor's `head_event_id`/`head_hash`/`event_count`. |
| `verify_chain` production callers | `crates/brokerd/src/confirmation.rs:599` (inside `confirm()`), `cli/caprun/src/main.rs:353` (end-of-run assertion) | **CONFIRMED — exactly 2 production call sites** | Small blast radius relative to `append_event`'s 19 — the signature change here is comparatively cheap. `print_audit_dag` (`main.rs:442-459`) does NOT call `verify_chain` — it is a separate, unauthenticated display-only walk; out of scope, no change needed. |
| **NEW FINDING — `deny()` has NO `verify_chain`/MAC check at all** | `crates/brokerd/src/confirmation.rs:753-785` | **CONFIRMED, gap not named in DESIGN §b blast radius** | `deny()`'s full body: `find_pending_confirmation` → terminal-state check → display → append `confirm_denied` → `transition_state`. No chain-verify, no digest recompute, nothing. Per the DESIGN doc's own X-02 ruling ("on every confirm/deny process start... MAC-re-verified or fail-closed re-derived"), this is a genuine scope question the plan must resolve explicitly — see Open Questions. |
| **NEW FINDING — `review()` also has NO `verify_chain` call** | `crates/brokerd/src/confirmation.rs:501-508` | **CONFIRMED** | `review()` is explicitly documented as a pre-decision, non-authoritative display (MAJOR-8) — this is likely FINE to leave unauthenticated (it never transitions state or invokes a sink), but note it explicitly in the plan rather than silently leaving it ambiguous. |
| `pending_confirmations` schema | `crates/brokerd/src/audit.rs:86-96` | **CONFIRMED, unchanged** | 9 columns: `effect_id, session_id, blocked_event_id, sink, resolved_args, blocked_arg_names, combined_digest, workspace_root_path, state`. The DESIGN pin folds `state` + `combined_digest` into the MAC scheme — a whole-row MAC (matching `events`' "whole-row-minus-side-tables" philosophy) is simpler to reason about than a 2-column MAC and avoids a partial-coverage gap on `resolved_args`/`blocked_arg_names` (which are equally forgeable today and equally load-bearing for `confirm()`'s Step 4.5b recompute-and-compare) — **recommend MAC'ing the FULL row**, not just the two named columns, unless the plan has a specific reason to narrow scope (flag as a discretionary call, not a locked one — the DESIGN doc's literal text names only `state`/`combined_digest`). |
| `transition_state` (the ONLY `pending_confirmations` mutator) | `crates/brokerd/src/confirmation.rs:296-306` | **CONFIRMED, exact lines match** | `UPDATE pending_confirmations SET state = ?1 WHERE effect_id = ?2 AND state = 'pending'` — this is where a NEW MAC column would need to be recomputed and rewritten alongside `state`, under the same statement/lock. `insert_pending_confirmation` (`:218-241`) is the sole INSERT site — where the INITIAL MAC gets computed. |
| `migrate_pending_confirmations_schema` — the idempotent-migration template to mirror | `crates/brokerd/src/audit.rs:120-145` | **CONFIRMED, exact lines match** | `PRAGMA table_info(pending_confirmations)` column-presence check, gated `ALTER TABLE ... ADD COLUMN` per missing column — never a blind `ALTER`. Phase 28's `chain_anchor` migration (for a pre-existing unauthenticated DB, per DESIGN "fail-closed migration" pin) should follow this EXACT idiom: check for the `chain_anchor` table's existence via `PRAGMA table_info` (or `sqlite_master` for a whole-table check, since `chain_anchor` is a NEW table, not a new column on an existing one — use `CREATE TABLE IF NOT EXISTS` in `SCHEMA_DDL` for the table shape, mirroring how `pending_confirmations`/`blocked_literals` were originally added, then a SEPARATE runtime check: does a `chain_anchor` row exist for a session whose `events` predate this migration? If not, that session's chain is "untrusted until re-anchored" per the DESIGN's fail-closed migration pin — `verify_chain` for such a session must return `false`, not silently pass.) |
| `open_audit_db` — the two cross-process call sites (KEY-CUSTODY, the highest-stakes finding) | `cli/caprun/src/main.rs:172-174` (the `caprun run` path, INSIDE an `Arc::new(Mutex::new(...))`) AND `cli/caprun/src/main.rs:384` (`run_confirm_or_deny`, a SEPARATE, LATER OS process) | **CONFIRMED, exact lines match — this is the load-bearing key-custody-across-processes seam** | `main()`'s `open_audit_db` call (line 173) happens BEFORE `workspace_root_dir` is derived (lines 182-193) — reordering or a deferred key-load step is needed so the F1 refusal check (which needs BOTH the audit/key path AND the workspace root) can run before the key is actually used. `run_confirm_or_deny`'s `open_audit_db` call (line 384) happens BEFORE the workspace root is known at all for `confirm` — that only becomes available via `pc.workspace_root_path` AFTER `find_pending_confirmation` returns `Some` (line 392-397). **Sequencing implication:** for `confirm`, the F1 refusal check + key load must be deferred until AFTER `find_pending_confirmation` succeeds (Step 1), but BEFORE `verify_chain` runs (Step 4.5a, line 599) — `deny` never learns the workspace root today at all (see the `deny()` gap finding above), which is a real wrinkle if `deny()` also needs the refusal-gated key. |
| `audit_path` free-form CLI arg | `main.rs:149` (positional-arg path) AND `main.rs:83-86` (the `confirm`/`deny`/`review` verb-dispatch path, defaults to `":memory:"`) | **CONFIRMED, exact lines match** | `:memory:` has no filesystem key-file location — the F1 refusal and key-generation logic must special-case `":memory:"` (skip the refusal check entirely, generate an ephemeral in-process-only key; no cross-process reuse is possible for `:memory:` anyway, confirmed by the existing comment at `main.rs:80-82`: "against :memory: no persisted row can exist, so this fails closed"). |
| `workspace_root_dir`/`workspace_rel`/`trusted_workspace_path` derivation | `main.rs:182-202` | **CONFIRMED, exact lines match, POST-Phase-27** | `workspace_root_dir = ws_path.parent()` (`:183-186`), `workspace_rel = ws_path.file_name()` (`:187-189`), `trusted_workspace_path: PathBuf = ws_path.to_path_buf()` (`:202`, Phase 27's HARDEN-01 addition, already forwarded into `run_broker_server`). Phase 28 needs `workspace_root_dir` (or an equivalent canonical form) available at the point the F1 refusal check runs — it is already computed here, just needs to be consulted before `open_audit_db`'s key-load step rather than only before the broker spawn. |
| `WorkspaceRoot::open`/`root_path()` | `crates/adapter-fs/src/workspace.rs:51-73` | **CONFIRMED, exact lines match** | `root_path()` returns the `PathBuf` the broker opened as its anchor — the F1 refusal's "canonicalize workspace root, canonicalize audit/key path, assert non-containment" check can use `WorkspaceRoot::open(...).root_path()` (or a plain `std::fs::canonicalize` on `workspace_root_dir` directly, without constructing a full `WorkspaceRoot` — the CLI already has `workspace_root_dir: &Path` at `main.rs:183` before `WorkspaceRoot::open` is even called at `:191`, so a plain `Path`-level check suffices and avoids threading a `WorkspaceRoot` value just for this refusal). |
| `run_broker_server` signature (Phase 27 baseline, for context) | `crates/brokerd/src/server.rs:163-172` | **CONFIRMED, unchanged since Phase 27** | 8 params today, including `trusted_workspace_path: PathBuf` (Phase 27's new hop). HARDEN-02 does NOT need a new `run_broker_server` parameter — the audit DB / key are entirely CLI-orchestrator (`main.rs`) concerns, opened and validated BEFORE `run_broker_server` is even spawned; the broker itself never re-derives or re-validates the key (it only ever sees the already-open `Arc<Mutex<Connection>>`). Confirmed no `run_broker_server` signature change is required for this phase — a smaller blast radius than Phase 27's session-status/trusted-path threading. |
| `append_event` — ALL 19 production call sites (the fanout the DESIGN doc's blast-radius note did not enumerate) | `server.rs:799,923,1046,1180,1263` (5); `quarantine.rs:371,416,491,781` (4); `confirmation.rs:550,656,717,776` (4); `sinks/file_create.rs:94,110,199,216` (4); `sinks/email_smtp.rs:260,293` (2) | **CONFIRMED — exactly 19, enumerated by grep this session, cross-checked against `fn append_event` definition to exclude it from its own count** | Every one of these already executes under an already-acquired `conn.lock()`/`&locked` guard (confirmed by reading the surrounding code at each server.rs/quarantine.rs/confirmation.rs site — see Architecture Patterns' atomicity discussion) — this is what makes "fold the chain_anchor upsert into `append_event` itself" viable: no caller needs its OWN new lock-acquisition or transaction-boundary change, only the ALREADY-flowing `key: &[u8]` needs to reach each of these 19 call expressions. `confirmation.rs:717` calls `append_event(&tx, ...)` where `tx` is a `rusqlite::Transaction` (from `conn.transaction()?` at `:701`, the ONLY SQL-transaction usage in the whole codebase) — `Transaction` derefs to `Connection`, so this call site is unaffected by the anchor-internalization choice; a chain_anchor row updated INSIDE `append_event` here would be part of that SAME SQL transaction automatically. |
| **CRITICAL FINDING — F1 refusal breaks 7 of 8 live test fixtures** | `cli/caprun/tests/{s9_live_block,e2e,live_acceptance_tainted_session,live_acceptance_v1_3,live_acceptance_v1_4_composed,llm_planner_live_accept,origin_seed_provenance}.rs` | **CONFIRMED, empirically verified this session by reading each file's directory-construction code** | ALL 7 either do `let workspace_file = tmp.join("workspace.txt"); let audit_db = tmp.join("audit.db");` (siblings in the SAME dir) or the equivalent `workspace_file = audit_db.parent().join(...)` pattern (`live_acceptance_v1_3.rs:101`, `live_acceptance_v1_4_composed.rs:265`) — either way, `workspace_root_dir = workspace_file.parent() == audit_db.parent()`, i.e. **`audit.db` is a direct child of the workspace root**, exactly the F1-vulnerable layout. Only `cli/caprun/tests/confirm.rs:221-223` (`workspace = tmp.join("workspace")` — a SUBDIRECTORY, distinct from `db_path = tmp.join("audit.db")`) is already safe. **Phase 28 MUST budget migrating all 7 vulnerable fixtures to a `confirm.rs`-style layout (workspace file under a subdirectory, audit DB as a sibling of that subdirectory, not of the file) as an explicit early task — this is not an incidental test-fixture tweak, it is the single largest concrete risk to Phase 28 landing green.** |
| `sink_blocked` Event / `combined_digest` field | `crates/runtime-core/src/event.rs:90,148-171` | **CONFIRMED** | `Event.combined_digest: Option<String>` is ALREADY part of the serialized `payload` column for `sink_blocked` events, so it's ALREADY covered by the events-table HMAC once that lands (no separate work needed for the `sink_blocked` event's own `combined_digest` field — only the SEPARATE `pending_confirmations.combined_digest` COLUMN, a different piece of data with the same name, needs its own new MAC per the DESIGN pin). |
| Tamper-simulation test inventory (regression surface, DESIGN's own risk callout) | `audit_dag.rs:94` (`tamper_breaks_chain`, raw `UPDATE events SET payload=...`); `durable_anchor.rs:338` (`tamper_evidence_mutating_payload_breaks_verify_chain`, `UPDATE events SET payload = REPLACE(...)`); `durable_anchor.rs:390` (`redacting_side_table_literal_preserves_verify_chain_and_digest`); `confirmation.rs:1558` (`tamper_event_payload_digest_inconsistently` helper) + `:1648` (`confirm_fails_closed_via_verify_chain_when_literal_and_event_digest_edited_self_consistently` — **THE exact self-consistent-rewrite forgery test the phase's Success Criterion 1 must keep failing-closed under the NEW keyed scheme**) + `:1707` (`digest_mismatch_then_retry_does_not_fork_dag_verify_chain_stays_true`) | **CONFIRMED — 6 named tamper-simulation tests, exhaustively enumerated by grep this session** | Every one of these does a raw `conn.execute("UPDATE events SET ...")` bypassing `append_event` entirely (simulating a bare DB-writer) — under the keyed scheme, these MUST continue to make `verify_chain` return `false`, since the attacker (by construction, a DB-writer without the key) cannot recompute a valid HMAC for their edited row. `confirm_fails_closed_via_verify_chain_when_literal_and_event_digest_edited_self_consistently` (`confirmation.rs:1648`) is explicitly the closest existing analog to Success Criterion 1's "internally self-consistent forgery" — re-verify this exact test (or a lineal descendant of it) still fails closed post-HMAC, and add a NEW test proving specifically that recomputing a SELF-CONSISTENT hash chain WITHOUT the key (i.e., simulating the unkeyed-forgery attack this phase closes) still fails, since today's test tampers the payload/digest DIRECTLY, not by "recomputing a plausible chain" the way an attacker with only `compute_event_hash`'s (public, unkeyed) algorithm could. |
| No existing `chain_anchor`/`hmac`/`getrandom`/`load_or_create_key` symbols anywhere in the tree | grepped `crates/`, `cli/` this session | **CONFIRMED — zero drift, clean slate** | Nothing pre-implemented for this phase; matches Phase 26's DESIGN-gate `git status --porcelain crates/ cli/` empty confirmation. |

## Architecture Patterns

### System Architecture Diagram

```
Process 1: `caprun <intent> <workspace-file> [audit-db-path]`  (main.rs)
  │
  │ 1. parse audit_path (:149) — may be a real path or ":memory:"
  │ 2. [NEW] derive workspace_root_dir EARLY (move ws_path.parent() logic
  │    up, or defer key-loading until after it's known — current code
  │    derives it at :182-193, AFTER open_audit_db at :172-174)
  │ 3. [NEW] load_or_create_key(audit_path, workspace_root_dir):
  │      - special-case ":memory:" -> generate ephemeral key, skip refusal
  │      - else: canonicalize workspace_root_dir AND audit_path AND
  │        audit_path+".key"; if either resolves BENEATH workspace_root_dir
  │        -> hard refuse (process exits, broker never spawns) [F1]
  │      - else: if <audit_path>.key exists, read it (0600); else
  │        getrandom::fill a fresh 32-byte key, write it 0600, return it
  │ 4. open_audit_db(&audit_path) — unchanged, schema DDL + migration
  │    (migration NOW also creates `chain_anchor` table + runs the
  │    fail-closed-on-legacy-DB check)
  │ 5. spawn run_broker_server(..., workspace_root, trusted_workspace_path)
  │    — NO new param needed here (F1's finding: the key/refusal is
  │    entirely a main.rs/CLI-orchestrator concern, never reaches the
  │    broker's own signature)
  │ 6. worker connects, RequestFd/ReportClaims/SubmitPlanNode as before —
  │    EVERY `append_event` call inside the broker now threads `&key`
  │    (19 production call sites) and, [RECOMMENDED] internally upserts
  │    `chain_anchor` under the SAME already-held conn.lock()
  │ 7. end of run: verify_chain(&locked, &session_id, &key) — NOW checks
  │    BOTH the recomputed HMAC walk AND the chain_anchor row's own MAC
  │    + event_count match (main.rs:353)

Process 2 (LATER, SEPARATE OS process): `caprun confirm <effect_id> [audit-db-path]`
  │
  │ 1. open_audit_db(audit_path) (:384) — same schema/migration path
  │ 2. find_pending_confirmation(&conn, effect_id) (:392) — Step 1, no
  │    key needed yet (just a deserializing SELECT)
  │ 3. [SEQUENCING WRINKLE] workspace_root_path is now known
  │    (pc.workspace_root_path) — [NEW] run the SAME F1 refusal check +
  │    load_or_create_key(audit_path, workspace_root_path) HERE, before
  │    Step 4.5a's verify_chain call (confirmation.rs:599)
  │ 4. confirm(&mut conn, effect_id, &ws, &key) — Step 4.5a now MAC-checks
  │    the chain_anchor too; Steps 1-2's pending_confirmations.state read
  │    [OPEN QUESTION] may need to move AFTER a pending_confirmations MAC
  │    check, not before, once that table is folded into the MAC scheme
  │    (today Step 2 trusts pc.state before any integrity check runs)
  │
  │ `caprun deny <effect_id>` — [GAP] currently has NO equivalent Step 3/4
  │    key-load or verify_chain call AT ALL (confirmation.rs:753-785) —
  │    Phase 28 must decide whether to add one (X-02's ruling says yes;
  │    the DESIGN §b blast-radius note only names confirm())
```

### Recommended Task Sequencing (informative — planner's call on final task boundaries)

1. **Test-fixture directory migration FIRST** (the F1-breaks-7-fixtures finding) — before ANY refusal logic lands, migrate `s9_live_block.rs`, `e2e.rs`, `live_acceptance_tainted_session.rs`, `live_acceptance_v1_3.rs`, `live_acceptance_v1_4_composed.rs`, `llm_planner_live_accept.rs`, `origin_seed_provenance.rs` to a `confirm.rs`-style layout (workspace file under its own subdirectory, audit DB as a sibling of THAT subdirectory). Run the full macOS `cargo test --workspace` BEFORE adding the refusal logic, to prove the migration itself is a no-op behaviorally (same tests pass, just relocated files) — this isolates "did the migration break anything" from "does the new refusal logic work," mirroring Phase 27's own "one signature-change pass" discipline of not conflating unrelated risk.
2. **Add `hmac`/`getrandom` to `crates/brokerd/Cargo.toml`** (root `[workspace.dependencies]` + crate reference, matching the existing `sha2`/`hex`/`nix` pattern) — a standalone, low-risk task, verifiable with `cargo build -p brokerd` alone.
3. **Key generation + `load_or_create_key` + F1 refusal helper** — a new function (likely in `audit.rs` or a new `key.rs` module) taking `audit_path: &str`, `workspace_root: &Path` and returning `Result<Vec<u8>>` (or a typed `AuditKey` newtype), special-casing `:memory:`. Unit-testable in isolation (no broker/worker needed) — write the F1 negative test (audit path under workspace root → refusal) here, before wiring it into `main.rs`.
4. **Keyed `compute_event_hash`/`append_event`/`verify_chain`** — the 19+2 call-site signature change. Do this as ONE pass (mirrors Phase 27's "one signature-change pass" lesson) — `cargo build --workspace` after this task catches every miss.
5. **`chain_anchor` table + migration + fold into `append_event`/`verify_chain`** — the atomicity-critical piece; write the tail-truncation-detection test here.
6. **`pending_confirmations` MAC fold** — extend `insert_pending_confirmation`/`transition_state`/`find_pending_confirmation` with a MAC column; decide (and document) the whole-row-vs-two-column scope call named above.
7. **Wire `main.rs`'s two `open_audit_db` call sites** to call `load_or_create_key` + the refusal check, in the correct sequence for each (run path: before spawn; confirm path: after `find_pending_confirmation`, before `verify_chain`).
8. **Resolve the `deny()` gap** — decide and implement per Open Questions.
9. **New tests**: self-consistent-forgery-without-the-key (Success Criterion 1), tail-truncation-via-anchor-mismatch, cross-process confirm still verifies (positive control — critical, since this is the DESIGN doc's #1 named risk: A2), F1 refusal negative test (if not already covered in step 3).

### Anti-Patterns to Avoid

- **A `thread_local!`/global static key.** Rejected in Alternatives Considered above — breaks explicit-parameter testability and diverges from the codebase's own style.
- **Computing the F1 refusal check ONLY once, at broker-spawn time, and skipping it in `run_confirm_or_deny`.** The DESIGN doc explicitly says "because the audit DB is opened at every `caprun` and every `confirm`/`deny` invocation, the shared key-load helper" must implement the refusal — this must be ONE shared function called from BOTH `main()` and `run_confirm_or_deny`, not duplicated ad hoc or skipped on the confirm/deny path (which has no live worker to protect against directly, but the invariant should hold uniformly — a future confirm/deny that DOES gain worker-adjacent behavior should not silently inherit an unguarded path).
- **Re-deriving `chain_anchor`'s `event_count` via SQL arithmetic (`event_count = event_count + 1`) inside a single UPSERT statement, then computing the MAC over a value you haven't read back.** The MAC must cover the ACTUAL new `event_count`; either read-then-write (safe under the already-held lock, no TOCTOU since single-writer-at-a-time) or use `RETURNING` (SQLite 3.35+, check `rusqlite`'s bundled SQLite version supports it) to get the new count back atomically in one statement — do not compute a MAC over a value you assumed rather than confirmed.
- **Skipping the migration test.** `migrate_pending_confirmations_schema`'s own test (`pending_confirmations_migration_widens_legacy_schema_idempotently`, `audit.rs:783-866`) is the template — a `chain_anchor` migration needs an equivalent test opening a pre-existing (pre-Phase-28) DB file and confirming: (a) the migration runs without erroring, (b) `verify_chain` on that legacy session returns `false` (fail-closed, per the DESIGN's "untrusted until re-anchored" pin) rather than panicking or silently passing.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HMAC construction | XOR-pad + double-SHA256 by hand | `hmac::Hmac<sha2::Sha256>` (`Mac` trait: `new_from_slice`, `update`, `finalize`/`verify_slice`) | Vetted, constant-time-safe, RustCrypto-ecosystem-standard; verified this session to compile against the existing `sha2 0.10` pin. |
| CSPRNG key material | A custom PRNG (even a "good-looking" one seeded from `SystemTime`) | `getrandom::fill(&mut buf)` | OS-CSPRNG-backed, already transitively resolved in the dependency graph via `uuid`; the DESIGN doc explicitly names "a vetted `getrandom`-backed RNG, never a custom PRNG." |
| Idempotent schema migration | A blind `ALTER TABLE ... ADD COLUMN` (errors if already run) or a version-number table | `PRAGMA table_info(...)` column-presence check before `ALTER`, exactly as `migrate_pending_confirmations_schema` already does | Already a proven, shipped, re-run-safe pattern in this exact codebase — copy the idiom for `chain_anchor`'s migration rather than inventing a new one. |
| Constant-time MAC comparison | `stored_mac == computed_mac` (a plain `==` on `String`/`Vec<u8>`, which is NOT constant-time in Rust's default `PartialEq`) | `hmac::Mac::verify_slice(&stored_mac_bytes)` (the `hmac` crate's own built-in constant-time compare) | A plain string/byte-vec `==` on a MAC value is a textbook timing-side-channel bug; `Mac::verify_slice` is exactly the API this crate provides to avoid hand-rolling a constant-time compare. **Do not port the existing `compute_event_hash != stored_hash` pattern's plain `!=` forward to the MAC comparison** — that pattern was fine for a non-secret unkeyed hash but is a new bug class once a secret-dependent MAC is involved. |

**Key insight:** Every cryptographic primitive this phase needs (HMAC, CSPRNG) has a vetted, already-compile-verified crate available at zero or near-zero new dependency cost — the actual engineering risk in this phase is NOT the cryptography, it's the **plumbing** (19-site `append_event` fanout, cross-process key custody sequencing, and the 7-test-fixture directory-layout landmine).

## Common Pitfalls

### Pitfall 1: The F1 refusal breaks 7 of 8 live test fixtures on first landing (CRITICAL — see anchor table)
**What goes wrong:** The broker (or CLI) refuses to start on `s9_live_block.rs`, `e2e.rs`, `live_acceptance_tainted_session.rs`, `live_acceptance_v1_3.rs`, `live_acceptance_v1_4_composed.rs`, `llm_planner_live_accept.rs`, and `origin_seed_provenance.rs` the moment the F1 refusal is implemented, because all 7 place `audit.db` directly beneath the workspace root by construction.
**Why it happens:** These fixtures were written across v1.1-v1.5 when co-locating the audit DB with the workspace file was harmless — the F1 vulnerability didn't exist as a concern until Phase 26's design-gate review found it.
**How to avoid:** Migrate all 7 fixtures' directory layout (workspace file under its own subdirectory) BEFORE landing the refusal logic — sequence this as an explicit, isolated, verifiable-on-its-own task (see Recommended Task Sequencing #1).
**Warning signs:** A wall of red Linux tests immediately after landing the refusal, all failing with the SAME "broker refused to start" error — if this happens, it means the fixture migration was skipped or done after rather than before.

### Pitfall 2: Key-custody mismatch — `confirm()` can never verify an earlier process's chain
**What goes wrong:** If the key is generated fresh per-process instead of loaded from the persisted `<audit_path>.key` sibling, EVERY legitimate `confirm()` call fails with `DigestMismatch` — not just an attack scenario, a total false-positive on the shipped `cli/caprun/tests/confirm.rs` test suite (`confirm_releases_once_and_second_confirm_is_already_terminal` et al., which spawn TWO separate `caprun` processes against the SAME `db_path`).
**Why it happens:** `caprun run` and `caprun confirm`/`deny` are always separate OS processes (no persistent broker daemon) — a naive "generate a random key in `open_audit_db`" implementation would silently generate a DIFFERENT key each time `open_audit_db` is called, defeating the entire mechanism.
**How to avoid:** `load_or_create_key` must be idempotent across processes: check for `<audit_path>.key` FIRST, only generate+write if absent. Use `O_CREAT|O_EXCL` (or equivalent) for the WRITE path to avoid a race if two processes somehow both try to create it simultaneously (unlikely in practice given `caprun run` always precedes `confirm`/`deny`, but cheap to guard).
**Warning signs:** `cli/caprun/tests/confirm.rs`'s cross-process tests all failing with exit code 8 (`DigestMismatch`) immediately after this phase lands.

### Pitfall 3: `append_event`'s 19-site fanout discovered via compile errors instead of budgeted upfront
**What goes wrong:** A developer changes `compute_event_hash`/`append_event`'s signature, fixes the handful of call sites they remember, and only discovers the remaining ~13 via a `cargo build --workspace` (or worse, a scoped `-p brokerd` build that misses the `sinks/` module's call sites, which live in the same crate but a different file the developer didn't think to grep).
**Why it happens:** Exactly Phase 27's Pitfall 1, but at a LARGER scale (19 vs. 11) — `append_event` is called from `server.rs`, `quarantine.rs`, `confirmation.rs`, AND two files under `sinks/` (`file_create.rs`, `email_smtp.rs`), a wider spread than Phase 27's single-function-family fanout.
**How to avoid:** Budget the exact 19-site list from the Anchor Table above as an explicit checklist item in the plan; run `cargo build --workspace` (not a scoped build) after the signature-change task.
**Warning signs:** Green `cargo build -p brokerd` but red `cargo test --workspace` (the SAME symptom Phase 27's research flagged for `dispatch_request`).

### Pitfall 4: Anchor write not genuinely atomic with the event it anchors
**What goes wrong:** If the `chain_anchor` upsert happens in a SEPARATE function call, invoked by the caller AFTER `append_event` returns (rather than internalized inside `append_event`), a caller could forget to call it, or a panic/early-return between the two calls could leave the anchor stale relative to the just-appended event — reopening exactly the truncation gap HARDEN-02 exists to close.
**Why it happens:** 19 call sites is a lot of places to remember a SECOND new call, versus one place (`append_event`'s own body) to add it once.
**How to avoid:** Fold the upsert into `append_event` itself (Recommendation #2, Architecture Patterns diagram) — every one of the 19 sites automatically gets anchor-atomicity "for free" once `append_event`'s signature grows the key param, with zero additional per-call-site code.
**Warning signs:** A test that appends an event then immediately (same lock scope) queries `chain_anchor` and finds it stale/missing — should never happen if internalized correctly; if it does, the anchor write was accidentally made a separate, skippable step.

### Pitfall 5: `pending_confirmations.state` trusted BEFORE its MAC is checked
**What goes wrong:** `confirm()`'s Step 2 (`if pc.state != Pending { return AlreadyTerminal }`, `confirmation.rs:576-578`) currently trusts `pc.state` read straight off disk, with no integrity check. If the MAC fold lands but the MAC-verification call is placed AFTER Step 2 (e.g., only added right before Step 4.5a's `verify_chain` call), an attacker who flips a `Denied` row back to `Pending` via raw SQL could still pass Step 2's check and reach the display/attempt-release logic before the MAC check catches the tamper later — a narrower window than today, but not the tightest possible fail-closed placement.
**Why it happens:** The DESIGN doc names `pending_confirmations` MAC folding as "re-checked at `confirm()`/`deny()` ENTRY alongside the chain-verify gate" — "entry" suggests BEFORE Step 2, not interleaved with Step 4.5a, but the two existing integrity gates (chain-verify, digest-recompute) both currently live well past Step 2 in the function body.
**How to avoid:** Place the `pending_confirmations` MAC check as EARLY as possible in `confirm()`/`deny()` — ideally immediately after `find_pending_confirmation` returns `Some(pc)`, before the terminal-state check reads `pc.state` for any decision-relevant purpose.
**Warning signs:** A new test — "flip a `Pending` row's `state` to `Denied` via raw SQL, then call `confirm()` on it" — passing for the WRONG reason (because Step 2's check happens to also reject it) rather than because the MAC check specifically caught the tamper; distinguish by also testing a flip-back TO `Pending` from `Denied`/`Confirmed`, which Step 2's plain `!= Pending` check would NOT catch on its own.

## Code Examples

### HMAC-SHA256 construction (verified this session against workspace `sha2 = "0.10"`)

```rust
// Source: hmac crate 0.12.1 docs (github.com/RustCrypto/MACs) — API verified
// this session via a scratch cargo check against the live workspace dependency
// graph (see Standard Stack / Package Legitimacy Audit).
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

fn compute_event_hash(
    key: &[u8],
    parent_hash: Option<&str>,
    id: &str,
    session_id: &str,
    event_type: &str,
    payload: &str,
    taint: &str,
) -> String {
    // new_from_slice accepts a key of ANY length (HMAC internally pads/hashes
    // it to the block size) — no fixed-length requirement, unlike some raw
    // block-cipher-keyed constructions.
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key)
        .expect("HMAC can take a key of any length");
    mac.update(parent_hash.unwrap_or("").as_bytes());
    mac.update(id.as_bytes());
    mac.update(session_id.as_bytes());
    mac.update(event_type.as_bytes());
    mac.update(payload.as_bytes());
    mac.update(taint.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

// Constant-time verification (do NOT hand-roll a `==` compare on the MAC):
fn verify_event_hash(key: &[u8], expected_hex: &str, /* same fields */ ) -> bool {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).unwrap();
    // ... same .update(...) calls ...
    let expected_bytes = match hex::decode(expected_hex) { Ok(b) => b, Err(_) => return false };
    mac.verify_slice(&expected_bytes).is_ok()
}
```

### Key generation (verified this session — `getrandom 0.4` API)

```rust
// Source: getrandom crate 0.4 docs (github.com/rust-random/getrandom) — API
// verified this session via a scratch cargo check; already transitively
// resolved in Cargo.lock via uuid's "v4" feature (zero new dependency
// resolution when added directly to crates/brokerd/Cargo.toml).
fn generate_mac_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    getrandom::fill(&mut key).expect("OS CSPRNG must be available");
    key
}
```

### Idempotent schema migration template to mirror (live, shipped precedent)

```rust
// Source: crates/brokerd/src/audit.rs:120-145 (live, shipped, re-verified this
// session) — the EXACT idiom Phase 28's chain_anchor migration should follow.
fn migrate_pending_confirmations_schema(conn: &rusqlite::Connection) -> Result<()> {
    let mut existing_columns: Vec<String> = Vec::new();
    {
        let mut stmt = conn.prepare("PRAGMA table_info(pending_confirmations)")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            existing_columns.push(name);
        }
    }
    if !existing_columns.iter().any(|c| c == "blocked_arg_names") {
        conn.execute(
            "ALTER TABLE pending_confirmations ADD COLUMN blocked_arg_names TEXT NOT NULL DEFAULT '[]'",
            [],
        )?;
    }
    // ... (mirror this shape for any new pending_confirmations MAC column,
    // and for a session-scoped "chain_anchor row exists?" fail-closed check
    // on legacy pre-Phase-28 DBs)
    Ok(())
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|---------------|--------|
| `verify_chain` recomputes an UNKEYED SHA-256 over each row and checks internal self-consistency only | `verify_chain` recomputes a KEYED HMAC-SHA256 (broker-secret) AND cross-checks a separately-MAC'd `chain_anchor` row for head/count | Phase 28 (this phase) | An in-host `events`-table writer can no longer forge a self-consistent chain OR silently truncate the tail — both previously-undetectable attacks (confirmed empirically: `verify_chain`'s `found_any` check has no expected-count assertion today) now fail closed. |
| `pending_confirmations.state`/`combined_digest` are plain, unauthenticated mutable columns | Folded into the same broker-key MAC scheme, re-checked at `confirm()`/`deny()` entry | Phase 28 (this phase) | Closes the flip-back/delete gap CONTEXT.md's X-02 named — a DB-writer can no longer silently resurrect a `Denied` row to `Pending` or vice versa. |
| Audit DB co-location with the workspace root was an unexamined convenience (7 of 8 live test fixtures do it) | The audit DB and its key file MUST resolve outside the workspace root, broker-enforced (F1) | Phase 28 (this phase), following the Phase-26 DESIGN-gate F1 finding | A confined worker can no longer `RequestFd` a co-located key file — but this makes the co-located test-fixture LAYOUT itself now-invalid, requiring the fixture migration flagged as Pitfall 1. |

**Deprecated/outdated:**
- The `DigestMismatch` doc comment's "HONESTY (MAJOR-6)" framing in `confirmation.rs:361-368` (which explicitly states `verify_chain`'s scope is "single-store and non-recomputing multi-store tampering... NOT authenticated/externally-anchored") becomes STALE the moment this phase lands and MUST be corrected in the same PR — mirrors Phase 27's requirement to fix `quarantine.rs`'s stale "SOLE I1 trust-flip site" comment in-PR, not as a follow-up. Same for the identical framing at `confirmation.rs:594-598` (Step 4.5a's own comment) and `audit.rs`'s module-doc comment referencing "See 03-RESEARCH.md Pattern 5" (unaffected, but the "hash chain" description at `audit.rs:1-15` should be updated to mention the key/anchor once this phase lands).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `hmac = "0.12.1"` (not the newest `0.13.0`) is the correct pin, because it compiles against the existing `sha2 = "0.10"` workspace dependency | Standard Stack | Low-Medium — empirically verified via `cargo check -p brokerd` this session (not merely asserted); the risk is narrow (a future `sha2` upgrade would need a paired `hmac` upgrade, a normal RustCrypto-ecosystem maintenance fact, not a correctness risk today). |
| A2 | Folding the `chain_anchor` upsert INSIDE `append_event` (rather than as a second call at each of 19 sites) is the recommended architecture | Architecture Patterns / Common Pitfalls #4 | Low — this is an engineering-judgment recommendation for minimizing blast radius and atomicity risk, not an explicit DESIGN-doc mandate (the DESIGN doc says "updated atomically with append_event," which is satisfied either way as long as atomicity holds); if the plan author prefers per-call-site anchor updates for some reason, the security property still holds AS LONG AS every one of the 19 sites is updated — flag as discretionary, not locked. |
| A3 | `pending_confirmations`'s MAC should cover the WHOLE row, not just `state`+`combined_digest` as the DESIGN doc's literal text names | Anchor Table (`pending_confirmations` schema row) | Medium — if the plan narrows scope to only `state`+`combined_digest` per the DESIGN doc's literal wording, `resolved_args`/`blocked_arg_names`/`workspace_root_path` remain forgeable by a bare DB-writer; `resolved_args` in particular is load-bearing for `confirm()`'s Step 4.5b recompute-and-compare (already gated by the SEPARATE `events`-chain `combined_digest` cross-check, so this may be an acceptable narrower scope — but the plan should state the choice explicitly, not silently default to one or the other). |
| A4 | `deny()` should gain the same `verify_chain`/MAC-check gate `confirm()` has, per the DESIGN doc's X-02 ruling | Anchor Table (`deny()` gap finding) / Open Questions | Medium-High — this is the single largest unresolved scope question this research surfaced; getting it wrong either under-delivers X-02's own ruling (if omitted) or adds unbudgeted work the DESIGN doc's §b blast-radius note didn't explicitly plan for (if added). See Open Questions for the recommended resolution path. |

## Open Questions

1. **Should `deny()` gain a `verify_chain`/`pending_confirmations`-MAC check, matching `confirm()`'s Step 4.5a/4.5b?**
   - What we know: The DESIGN doc's X-02 cross-cutting ruling explicitly names BOTH "confirm/deny process start" as needing MAC-re-verified or fail-closed-re-derived recovered state. The current code (`confirmation.rs:753-785`) has ZERO such check in `deny()` — confirmed by direct reading, not by memory or the DESIGN doc's prose.
   - What's unclear: The DESIGN doc's §b "Blast radius (Phase 28)" section only names `confirmation.rs`'s "confirm() Step 4.5a" — it does not mention `deny()` at all, suggesting either an oversight in the DESIGN doc's own blast-radius note, or an implicit assumption that `deny()` doesn't need it because it never releases a sink effect (a corrupted `deny()` outcome is "fail-safe" in the sense that no effect fires — but a flipped-back `state` could still let an attacker force-terminate a confirmation a human might have legitimately confirmed, denying service rather than escalating privilege).
   - Recommendation: Add the SAME gate to `deny()` as `confirm()` gets — X-02's ruling is a LOCKED cross-cutting decision from a cleared design gate, and the DESIGN doc's own blast-radius note is informative, not authoritative, per its own framing ("§i — informative — not part of the gate"). This is a case where a locked ruling (§f/X-02) should win over an informative implementation note (§b's blast-radius list) that appears to have simply missed enumerating `deny()`. Flag this explicitly to the plan author as the single highest-value scope decision this research surfaced.

2. **Should `pending_confirmations`'s new MAC cover the whole row or only `state`+`combined_digest`?**
   - What we know: The DESIGN doc's literal text names `state`+`combined_digest`. The Anchor Table above notes `resolved_args`/`blocked_arg_names`/`workspace_root_path` are equally forgeable and equally load-bearing today.
   - What's unclear: Whether the DESIGN doc's narrower naming was a deliberate scope decision (defense-in-depth via the `events`-chain `combined_digest` cross-check already covering `resolved_args`' integrity indirectly) or simply the two columns most salient to the flip-back/delete gap the doc was focused on.
   - Recommendation: MAC the whole row (Assumption A3) — marginal extra cost, closes a strictly larger attack surface, and avoids a future re-litigation of "wait, was `resolved_args` supposed to be covered too?"

3. **Exact placement of the `pending_confirmations` MAC check relative to `confirm()`/`deny()`'s existing Step 1 (fetch) / Step 2 (terminal-state check).**
   - What we know: The DESIGN doc says "re-checked at `confirm()`/`deny()` entry" — suggesting as early as possible.
   - What's unclear: Whether "entry" means literally before Step 1 (impossible — you need the row to check its MAC) or immediately after Step 1's fetch, before Step 2 reads `pc.state` for a decision (Pitfall 5's concern).
   - Recommendation: Immediately after Step 1's `find_pending_confirmation` returns `Some(pc)`, before Step 2's terminal-state branch — matches the DESIGN doc's "entry" framing most literally while closing Pitfall 5's window.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust/Cargo toolchain | All Phase 28 work | Confirmed present (used this session for `cargo check`/`cargo search`/`cargo info`) | — | N/A (hard requirement) |
| crates.io registry access | `cargo search`/`cargo info`/dependency resolution for `hmac`/`getrandom` | Confirmed present (used this session, live queries) | — | none needed |
| Colima + Docker (`rust:1` image) | Linux-only forgery-rejection/F1-refusal/cross-process tests via `scripts/mailpit-verify.sh` | Per CLAUDE.md, installed on this dev machine | — | macOS `cargo test --workspace` runs everything else; Linux-gated tests show "0 passed" (expected) until run via the script |
| `scripts/mailpit-verify.sh` | Live-Linux verification (same standing rule since Phase 16 — even a "looks benign" test may trigger a real SMTP send) | Present in repo (confirmed) | — | none needed |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** none — everything this phase needs is already available or empirically verified to resolve cleanly this session.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `cargo test` (workspace-wide) + Cargo integration-test binaries under each crate's `tests/` dir |
| Config file | none (Cargo.toml `[dependencies]`/`[dev-dependencies]` — no separate test-runner config) |
| Quick run command | `cargo build --workspace` (catches the 19+2-site signature-mismatch compile errors fast) then `cargo test -p brokerd --no-fail-fast` |
| Full suite command | `cargo test --workspace --no-fail-fast` (macOS — Linux-gated tests no-op); `bash scripts/mailpit-verify.sh` (Linux, full live proof) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| HARDEN-02 (Success Criterion 1) | A self-consistent-rewrite forgery (event row rewritten, every descendant hash/parent_hash recomputed self-consistently) is rejected — WITHOUT the key, an attacker cannot recompute a valid HMAC | unit/integration | `cargo test -p brokerd --no-fail-fast -- forge_without_key` (new test name TBD) | ❌ Wave 0 — new test; closest existing analog is `confirmation.rs:1648`'s `confirm_fails_closed_via_verify_chain_when_literal_and_event_digest_edited_self_consistently`, which tampers via a raw `UPDATE events SET payload = ?1` bypassing the key entirely — re-verify this EXISTING test still fails closed post-HMAC (it should, automatically, since the attacker never has the key), then ALSO add a new test proving the specific "recompute EVERY descendant's hash to be internally self-consistent" scenario the phase's success criterion names verbatim. |
| HARDEN-02 (Success Criterion 2) | The chain's authenticity depends on a secret key OR out-of-store anchor a bare `events`-writer cannot derive/reproduce | unit | `cargo test -p brokerd --no-fail-fast -- key_dependence` (new) | ❌ Wave 0 — new test: construct two DIFFERENT keys, show the SAME event data produces DIFFERENT MACs, and that `verify_chain` with the WRONG key fails even on an otherwise-untampered chain. |
| HARDEN-02 (tail-truncation) | A `DELETE`-the-tail attack (previously undetectable per the confirmed `found_any`-only check) is now caught via the `chain_anchor`'s `event_count`/`head_hash` mismatch | integration | `cargo test -p brokerd --no-fail-fast -- tail_truncation` (new) | ❌ Wave 0 — new test, DB-alone (mirrors `audit_dag.rs`'s existing `tamper_breaks_chain` style: raw `DELETE FROM events WHERE ...` then assert `verify_chain` false). |
| HARDEN-02 (Success Criterion 3) | An untampered chain still verifies true — no false positives; existing confirm-path/live-acceptance callers unaffected | regression | `cargo test --workspace --no-fail-fast` (macOS) then `bash scripts/mailpit-verify.sh` (Linux) | ✅ existing — the 6 named tamper-simulation tests (Anchor Table) plus ALL `verify_chain`-asserting live-acceptance tests across `cli/caprun/tests/*.rs` and `crates/brokerd/tests/*.rs` are the regression surface; re-run in full, not spot-checked. |
| HARDEN-02 (F1 refusal) | Audit path (or its `.key` sibling) resolving beneath the workspace root → broker/CLI refuses to run, fail-closed | unit/integration (Linux-gated for the `RESOLVE_BENEATH` semantics, though the canonicalize-and-compare check itself is portable) | `cargo test -p brokerd --no-fail-fast -- f1_refusal` or `-p caprun` depending on where the check lands (main.rs) | ❌ Wave 0 — new test, per DESIGN §g's own explicit requirement ("Phase 28 ... MUST implement and test this refusal path"). |
| HARDEN-02 (cross-process key custody, DESIGN's #1 named risk A2) | `caprun run` then a SEPARATE `caprun confirm` process on the same DB still verifies true (positive control) | integration, real subprocess spawn | `cargo test -p caprun --test confirm --no-fail-fast` | ✅ existing — `cli/caprun/tests/confirm.rs`'s cross-process tests are the EXISTING regression canary for this exact risk; they must continue passing unchanged (their directory layout is already F1-safe — see Anchor Table). |

### Sampling Rate
- **Per task commit:** `cargo build --workspace` (fast compile-error catch across all 19+2 call sites) + scoped `cargo test -p brokerd`
- **Per wave merge:** `cargo test --workspace --no-fail-fast`
- **Phase gate:** Full suite green on macOS at minimum; Linux `bash scripts/mailpit-verify.sh` pass required before marking Phase 28 done, given this phase directly touches the confirm-path tests `scripts/mailpit-verify.sh` exists specifically to gate (per CLAUDE.md's standing rule since Phase 16).

### Wave 0 Gaps
- [ ] Test-fixture directory migration for the 7 vulnerable files (Pitfall 1) — prerequisite, not itself a NEW test, but must land before the F1 refusal's own tests can be trusted not to be masking a fixture-layout failure.
- [ ] New test: self-consistent-rewrite forgery WITHOUT the key fails closed (Success Criterion 1, verbatim).
- [ ] New test: key-dependence (two different keys → two different MACs; wrong key → verify_chain false on an untampered chain).
- [ ] New test: tail-truncation caught via `chain_anchor` mismatch.
- [ ] New test: F1 refusal (audit/key path beneath workspace root → refuse).
- [ ] New test (if Open Question 1 resolves "yes"): `deny()` gains the same MAC/verify-chain gate `confirm()` has.
- [ ] New test: legacy (pre-Phase-28) DB with no `chain_anchor` row → `verify_chain` fails closed (untrusted-until-re-anchored migration pin).
- [ ] Verify (don't assume) the 6 existing tamper-simulation tests + `confirm.rs`'s cross-process tests still pass for the RIGHT reason post-HMAC (mirrors Phase 27's A4 empirical-verification discipline) — not simply "green," but green because the key-dependent logic is actually exercised, not because a test happens to hit an unrelated fail-closed path.

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | N/A — no user-facing auth surface |
| V3 Session Management | yes | `chain_anchor`/keyed-MAC directly hardens the audit substrate underlying `caprun`'s session-trust-state model — a broken/forged chain must never be silently trusted as evidence of a session's history. |
| V4 Access Control | no (not directly) | This phase does not touch `ConnectionRole`/capability gating (Phase 20/27 scope) — the F1 refusal is a filesystem-reachability check, adjacent to but distinct from connection-role access control. |
| V5 Input Validation | no (not directly) | No new worker-controlled input surface — `RequestFd { path }` validation is unchanged (Phase 27 scope); this phase's new surface (the `.key` file, `chain_anchor` table) is entirely broker/CLI-internal, never worker-supplied. |
| V6 Cryptography | **yes — this phase's primary category** | HMAC-SHA256 via the vetted `hmac` crate (never hand-rolled — see Don't Hand-Roll); CSPRNG key generation via `getrandom` (never a custom PRNG); constant-time MAC comparison via `Mac::verify_slice` (never a plain `==`/`!=` on the MAC value — a NEW risk this phase introduces relative to the existing unkeyed-hash `!=` pattern, which was fine for a non-secret value but is a timing-side-channel bug class once a secret-dependent MAC is involved). |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| In-host DB-writer forges a self-consistent hash chain (no key needed today) | Tampering | Keyed HMAC — a bare DB-writer cannot recompute a valid MAC without the key, which lives outside both the DB file and the confined worker's Landlock-reachable filesystem tree. |
| Tail-truncation (DELETE the last N events, chain still "verifies" via the shorter true leaf) | Tampering / Repudiation | MAC'd `chain_anchor` (session_id, head_event_id, head_hash, event_count) updated atomically with every append — an attacker cannot re-MAC the anchor without the key, so a truncated tail no longer matches the anchor's expected count/head. |
| Confined worker reads the MAC key via `RequestFd` if co-located under the workspace root | Elevation of Privilege / Spoofing | Broker/CLI-enforced fail-closed startup refusal (F1) — canonical-path containment check before the key is ever used, refusing to run rather than silently trusting operator convention. |
| Timing side-channel on MAC comparison (`==`/`!=` instead of constant-time compare) | Information Disclosure | `hmac::Mac::verify_slice` (constant-time by construction) — NEVER port the existing unkeyed-hash's plain `!=` comparison pattern forward to the new MAC check. |
| `pending_confirmations.state`/`combined_digest` flip-back via raw SQL (bypassing `transition_state`'s `WHERE state='pending'` guard) | Tampering | Fold into the same broker-key MAC scheme, checked at `confirm()`/`deny()` entry BEFORE the terminal-state branch trusts `pc.state` for any decision (Pitfall 5). |

## Sources

### Primary (HIGH confidence — direct codebase read + empirical compile verification, this session)
- `crates/brokerd/src/audit.rs` — full file read: schema DDL, `migrate_pending_confirmations_schema`, `open_audit_db`, `compute_event_hash`, `append_event`, `verify_chain`, all 6 tamper-simulation-adjacent test helpers.
- `crates/brokerd/src/confirmation.rs` — `combined_digest`, `ResolvedArg`, `find_pending_confirmation`, `transition_state`, `ConfirmOutcome`, `confirm()` (full Step 1-4.5b), `deny()` (full body — confirmed NO verify_chain call), `review()` (confirmed NO verify_chain call), `insert_pending_confirmation`.
- `crates/brokerd/src/server.rs` — all 5 `append_event` call sites, `run_broker_server` signature (Phase 27 baseline, confirmed unchanged for this phase's needs), the RequestFd/session_status Phase 27 machinery (confirmed zero drift).
- `crates/brokerd/src/quarantine.rs` — `mint_from_read` full body (2 `append_event` calls, the atomicity-via-lock precedent), 2 additional `append_event` call sites elsewhere in the file.
- `crates/brokerd/src/sinks/file_create.rs`, `crates/brokerd/src/sinks/email_smtp.rs` — 4 + 2 `append_event` call sites respectively.
- `crates/brokerd/src/session.rs` — `update_session_status`/`persist_session`, confirmed `sessions.status` is never read back by `confirm()`/`deny()` (relevant to the X-02 framing nuance).
- `crates/adapter-fs/src/workspace.rs` — `WorkspaceRoot::open`/`root_path`/`read_within`, confirms the F1 refusal's canonicalization target.
- `cli/caprun/src/main.rs` — full `main()` + `run_confirm_or_deny` read: `audit_path` parsing (both verb-dispatch and positional-arg paths), `open_audit_db` at BOTH call sites, `workspace_root_dir`/`workspace_rel`/`trusted_workspace_path` derivation, `verify_chain` end-of-run assertion, `run_confirm_or_deny`'s per-verb dispatch.
- `crates/runtime-core/src/event.rs` — `Event.combined_digest` field, `Event::sink_blocked` constructor, confirmed the field is already inside the hashed `payload`.
- `crates/brokerd/Cargo.toml`, root `Cargo.toml`, `Cargo.lock` — confirmed `sha2`/`hex` present, `hmac`/`getrandom`/`rand` absent as direct deps but `getrandom 0.4.3`/`rand 0.10.2` already transitively resolved via `uuid`.
- **Empirical compile verification (this session, then fully reverted — `git status --porcelain` confirmed clean):** `hmac = "0.12.1"` + a scratch `Hmac<Sha256>` usage compiles against `sha2 = "0.10"`; `getrandom = "0.4"` + a scratch `getrandom::fill` usage compiles and resolves against the existing lockfile.
- `gsd-tools query package-legitimacy check --ecosystem crates hmac getrandom` — both `OK`, high download counts, canonical repos.
- Exhaustive grep across `crates/brokerd/src`, `crates/brokerd/tests`, `cli/caprun/tests` for `append_event(`, `verify_chain`, `tamper`, `UPDATE events` — the basis for the 19-site fanout count, the 2-site `verify_chain`-caller count, and the 6-test tamper-simulation inventory.
- Directory-construction code in all 8 `cli/caprun/tests/*.rs` files referencing `workspace_file`/`audit_db` — the basis for the F1-breaks-7-fixtures finding.
- `planning-docs/DESIGN-security-hardening.md` (full §b + §f/X-02 + §g/§h/§i), `planning-docs/DESIGN-GATE-RECORD-v1.6.md` (F1 finding + resolution) — the authoritative design contract, re-read in full this session.
- `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `.planning/STATE.md` — phase requirement text, success criteria, project history.
- `.planning/phases/27-session-connection-integrity-hardening/27-RESEARCH.md` — prior-phase research read in full for style/format precedent and to confirm Phase 27's landed state (all anchors cross-checked against live source, zero drift found).
- `./CLAUDE.md` — project-specific hard constraints.

### Secondary (MEDIUM confidence)
- `cargo search`/`cargo info` output for `hmac`/`sha2`/`getrandom` — live crates.io registry queries this session, not training-data memory (used to determine current latest versions and confirm `hmac 0.12.1` vs `0.13.0` compatibility empirically rather than by inference).

### Tertiary (LOW confidence)
- None — no WebSearch or non-authoritative source was used this session; every claim traces to direct codebase reads, live registry queries, or empirical compile tests.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — both new dependencies empirically compile-tested against the live workspace graph, not merely asserted from training data; legitimacy-checked via the package-legitimacy seam.
- Architecture: HIGH — every anchor re-verified against live source post-Phase-27; the 19-site `append_event` fanout and the 7-fixture F1 vulnerability were discovered via direct grep/read this session, not inferred from the DESIGN doc's prose (which did not name either at this granularity).
- Pitfalls: HIGH — all 5 pitfalls are grounded in concrete, re-verified code facts (exact call-site counts, exact test-fixture directory-construction code, the exact `deny()`/`review()` code bodies proving the absence of a verify_chain call) rather than generic domain knowledge.

**Research date:** 2026-07-12
**Valid until:** Effectively immediate — this research is anchor-verification against the CURRENT tree state (post-Phase-27, zero drift found). If any commits land in `crates/brokerd/`, `cli/caprun/src/main.rs`, or `cli/caprun/tests/` before Phase 28 executes, re-run the grep/read verification commands in this doc's Anchor Table before trusting the line numbers and call-site counts (mirrors the DESIGN doc's and Phase 27's own re-verification discipline).
