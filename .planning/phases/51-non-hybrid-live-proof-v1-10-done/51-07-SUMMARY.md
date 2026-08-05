---
phase: 51-non-hybrid-live-proof-v1-10-done
plan: 07
subsystem: audit-integrity
tags: [rust, sqlite, audit-chain, concurrency, tamper-evidence]

requires:
  - phase: 51-05
    provides: Committed RED audit-chain fork and contention regressions
  - phase: 51-06
    provides: Append-at-head concurrency design contract and production call-site inventory
provides:
  - Serialized append-at-durable-head audit choke point under BEGIN IMMEDIATE
  - Explicit TCB-owned SQLite busy timeout on every audit connection
  - Mechanical proof that verifier bytes and provenance handling did not move
affects: [51-08-adversarial-review, 51-04-live-proof-rerun, LIVE-07, LIVE-08]

tech-stack:
  added: []
  patterns: [durable-head selection under write lock, re-entrant transaction joining]

key-files:
  created: []
  modified:
    - crates/brokerd/src/audit.rs
    - crates/brokerd/src/server.rs
    - crates/brokerd/src/confirmation.rs
    - planning-docs/DESIGN-audit-append-concurrency.md
    - planning-docs/DESIGN-GATE-RECORD-v1.10.md

key-decisions:
  - "The durable head selected inside the write transaction is authoritative; caller parent state remains API-compatible but advisory."
  - "User-authorized amendment: append_event's immutable Connection branch uses Transaction::new_unchecked with Immediate behavior, guarded by is_autocommit; mutable enclosing sites use transaction_with_behavior."
  - "The independent adversarial-review gate remains PENDING for Plan 51-08."

patterns-established:
  - "Audit appends select, hash, insert, count, and anchor under one immediate transaction."

requirements-completed: [LIVE-07, LIVE-08]

coverage:
  - id: D1-D2
    description: External and contended audit appends remain one verifiable linear chain.
    requirement: LIVE-07
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/audit_chain_fork_regression.rs (2/2 passed)"
        status: pass
    human_judgment: false
  - id: verifier-provenance
    description: Verifier/MAC functions are byte-identical and provenance graph content is untouched.
    requirement: LIVE-08
    verification:
      - kind: other
        ref: "Task 2 mechanical Gates A-C plus extract_provenance_threading (4/4 passed)"
        status: pass
    human_judgment: false

duration: 39min
completed: 2026-08-05
status: complete
---

# Phase 51 Plan 07: Atomic Audit Append-at-Head Repair Summary

**Every audit append now derives its causal parent under an immediate SQLite write boundary, with explicit contention handling and unchanged verifier/provenance contracts.**

## Performance

- **Duration:** 39 min active execution (excluding sandbox approval stalls)
- **Completed:** 2026-08-05
- **Tasks:** 3
- **Production files modified:** 3

## Accomplishments

- Moved durable-head selection, event hashing/insertion, persisted count read-back, and chain-anchor upsert into one immediate transaction while preserving root and re-entrant behavior.
- Made the 5000 ms audit busy timeout explicit in `open_audit_db` and converted all three enclosing append transactions to immediate behavior in the same production commit.
- Flipped both committed Plan 51-05 RED regressions green without editing them and mechanically proved the verifier, MAC primitives, provenance graph, mint surface, and dependency surface did not move.

## Task Commits

1. **User-authorized design-contract amendment** — `a6b8911`
2. **Task 1: Land the coordinated D1+D2 fix as one atomic commit** — `442a056`
3. **Tasks 2-3: Mechanical and scoped verification** — no source changes by design

## Files Created/Modified

- `crates/brokerd/src/audit.rs` — explicit timeout and transactional append-at-durable-head choke point.
- `crates/brokerd/src/server.rs` — two enclosing transactions changed from deferred to immediate behavior.
- `crates/brokerd/src/confirmation.rs` — attempt-ledger enclosing transaction changed to immediate behavior.
- `planning-docs/DESIGN-audit-append-concurrency.md` — records the authorized immutable-connection transaction mechanism.
- `planning-docs/DESIGN-GATE-RECORD-v1.10.md` — records the amendment without clearing the PENDING independent-review gate.

## Verification

### Task 1 — RED to GREEN and atomicity

- Pre-fix `cargo test -p brokerd --test audit_chain_fork_regression -- --test-threads=1`: exit 101; both tests failed as committed RED oracles (2 leaves and 48 leaves respectively).
- Post-fix same command: exit 0; 2 passed, 0 failed.
- `./scripts/check-invariants.sh`: exit 0; Gates 1-6 PASSED.
- `cargo build --workspace`: exit 0.
- `git show --stat 442a056`: exactly `audit.rs`, `server.rs`, and `confirmation.rs`; 60 insertions, 29 deletions.
- Mechanical counts: audit Immediate `1`; server `transaction_with_behavior` `2`; confirmation `transaction_with_behavior` `1`; audit `busy_timeout` `1`.

### Task 2 — mechanical non-regression gates (verbatim terminal result)

```text
Gate 1: checking for raw effect-to-sink type in crates/ ...
  PASS — no raw effect-to-sink type found in crates/
Gate 2: checking runtime-core purity ...
  PASS — runtime-core is pure (no I/O, no async, no network)
Gate 3: checking mint-call-site restriction (mint_from_read / mint_from_derivation / mint_from_exec / mint_from_http / .mint()) ...
  PASS — mint_from_read / mint_from_derivation / mint_from_exec / mint_from_http / .mint() restricted to sanctioned loci
Gate 4: checking test-fixtures is never a default feature of crates/brokerd ...
  PASS — test-fixtures is not a default feature of crates/brokerd
Gate 4b: checking mock-egress-ca is never a default feature anywhere in the workspace ...
  PASS — mock-egress-ca is not a default feature of any workspace member
Gate 5: checking aws-lc-rs is absent from the workspace build graph ...
  PASS — aws-lc-rs absent from the workspace build graph (ring-only crypto)
  PASS — no openssl-sys via reqwest (only lettre's native-tls path is allowed)
Gate 6: checking containment-predicate anti-drift (single shared helper + delegation) ...
  PASS — containment predicate lives only in the shared helper; all consumers delegate

All invariant gates PASSED.
GATES A-C PASSED
```

- Gate A extracted non-empty bodies for `verify_chain`, `current_chain_head`, `compute_event_hash`, `verify_event_hash`, `compute_anchor_mac`, and `verify_anchor_mac`; every body was byte-identical to commit `c3bad5e`.
- Gate B found zero changed content lines mentioning `read_event_id` or `provenance_chain` under `crates/brokerd/src/`.
- Gate C found no added `mint_from_` or forbidden raw effect-to-sink token and no Cargo manifest/lockfile change.
- Gate D — `record_github_grant`: **FIXED BY CONSTRUCTION** because its advisory read remains, but `append_event` rebinds the persisted parent under its immediate transaction.
- Gate D — dual-connection same-seed head: **FIXED BY CONSTRUCTION** because every production append enters the same choke point and no call site can persist its threaded in-memory parent.
- Task 2 working diff was empty for source files.

### Task 3 — scoped Docker-less regression

- Broker family command: exit 0 on the authoritative unrestricted rerun; audit-chain 2/2, audit DAG 2/2, durable anchor 5/5, provenance threading 4/4, session integrity 3/3, and section-9 acceptance 5/5 passed. The first restricted-subagent attempt reached audit-chain/audit-DAG green, then hit host `EPERM` at the durable-anchor dispatch boundary; the identical unrestricted rerun resolved this as environmental.
- CLI family command: exit 0; audit viewer 4/4, grant 2/2, planner 25/25, selector rejection 1/1, file-write block 3/3, and process-exec block 6/6 passed.
- LIVE compile gate `cargo test -p caprun --test live_acceptance_v1_10_cli --features live-proof-fixtures,mock-egress-ca --no-run`: exit 0.
- Final `./scripts/check-invariants.sh`: exit 0; Gates 1-6 PASSED.
- Final `git diff --check`: exit 0.

The full-workspace compose regression was intentionally **not run on this host** and remains the unchanged obligation of Plan 51-04. Broken windows 1, 2, and 3 remain open. No Docker, compose-verify, mailpit-verify, or bare workspace-test command was invoked by Task 3.

## Decisions Made

- Preserved the public `append_event(&Connection, ...)` signature. After the original contract was found incompatible with rusqlite 0.32.1's mutable receiver, the user explicitly authorized `Transaction::new_unchecked(conn, Immediate)` only in the `is_autocommit` branch.
- Kept all verifier, MAC, provenance, anchor, replay-suppression, and sink-blocked ordering behavior unchanged.

## Deviations from Plan

### User-authorized contract amendment

- **Found during:** Task 1 pre-implementation API check.
- **Issue:** rusqlite 0.32.1's `Connection::transaction_with_behavior` requires `&mut Connection`, contradicting the locked unchanged `append_event(&Connection, ...)` signature.
- **Resolution:** User selected the documented immutable-connection immediate transaction mechanism; design and gate records were amended in `a6b8911` before production code landed.
- **Impact:** Mechanism-only clarification. The immediate boundary, explicit re-entrancy branch, public signature, and every security invariant remain intact.

## Issues Encountered

- The subagent sandbox made `.git` read-only and restricted a durable-anchor dispatch operation. The orchestrator performed the exact three-file commit and authoritative identical broker regression outside that sandbox.
- `cargo fmt` was unavailable because the rustfmt component is not installed. Compiler, test, and `git diff --check` gates passed; no package/component installation was attempted.

## Authentication Gates

None.

## Known Stubs

None.

## Threat Flags

None beyond the audit TCB surface explicitly registered and mitigated in Plan 51-07's threat model.

## Next Phase Readiness

Plan 51-08 can run the required fresh non-self adversarial trace against `442a056`. The gap-closure gate remains PENDING, and LIVE-07/LIVE-08 completion still depends on the unchanged authoritative Plan 51-04 real-Linux compose rerun.

## Self-Check: PASSED

- All five modified implementation/design files exist.
- Commits `a6b8911` and `442a056` are reachable.
- Production commit `442a056` contains exactly the three declared broker source files.
- No source diff remains after Tasks 2-3.
- `.planning/STATE.md`, ROADMAP, REQUIREMENTS, WINDOWS, validation, LIVE evidence, and completion ledgers were not edited by this executor.

---
*Phase: 51-non-hybrid-live-proof-v1-10-done*
*Completed: 2026-08-05*
