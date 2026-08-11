# Phase 51 real-Linux LIVE evidence

## Environment and source identity

The following host identity and timing fields are executor-supplied metadata. They were
recorded during provisioning/execution but were not emitted into either hashed compose
log. The retained logs independently establish real-Linux Docker execution at the named
source revision through the Linux-only test roster, container build transcript, and the
absence of later source/test changes.

- Execution host: temporary AWS EC2 `c7i.4xlarge` instance `i-0fb4d663c0097fdc5` in account `559846026666`, tagged `Project=caprun`.
- Operating system: Ubuntu 24.04 LTS, Linux `6.17.0-1019-aws`, `x86_64`.
- Docker: client/server `29.7.2`; Docker Compose `v5.4.0`.
- Git commit: `cb34b9124916164397697a948c0c4804db221c82`.
- Scoped completion time: `2026-08-08T16:41:47Z` (UTC).

## Scoped LIVE-07/LIVE-08 gate

Exact command:

```sh
COMPOSE_VERIFY_CMD='cargo build --workspace && cargo test -p caprun --test live_acceptance_v1_10_cli --features live-proof-fixtures,mock-egress-ca -- --test-threads=1' bash scripts/compose-verify.sh
```

- Exit status: `0`, preserved through a `bash -o pipefail` tee.
- Complete stdout/stderr: `51-LIVE-SCOPED.log`.
- SHA-256: `dc57e49b2d75ec0d040da69a780f13485507148b5efe19d0457640d41916b98d`.
- Result excerpt: `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` and `Composed Linux verification suite PASSED (Mailpit + mock GitHub).`

The hashed scoped log records this test as passing, but `cargo test` suppresses stdout for
passing tests; the individual assertion strings below therefore do **not** appear in the
log. At executed revision `cb34b91`, the passing
`live_07_cli_multi_node_one_session_verify_chain` test necessarily satisfied the hard
assertions in `cli/caprun/tests/live_acceptance_v1_10_cli.rs`: driver exit `0`, exactly
one surfaced Session, exactly one durable `git_push_succeeded` event, exactly one durable
`github_pr_succeeded` event, and `assert_audit_passed`, which requires `caprun audit` exit
`0` and stdout containing `Chain verification: PASSED` (the LIVE-07 test body and
`assert_audit_passed` helper at executed-revision lines 405–429 and 343–356,
respectively).

Likewise, the hashed log records the sibling
`live_08_cli_mid_loop_i2_block_genuine_taint` test as passing but does not contain its
suppressed assertion output. At executed revision `cb34b91`, that pass necessarily means
the test satisfied its hard assertions: exactly one Session, no durable
`github_pr_succeeded` event, exactly one durable `sink_blocked` anchor for sink
`github.pr` and argument `body`, non-empty taint containing an untrusted label, and an
anchor `read_event_id` that selects the sole `sink:process.exec:*` event and equals the
first provenance root. The same helper pins exactly two exit events—one
`sink:process.exec:*` and one `sink:git.commit:*`—so the commit event cannot be credited.
Its `assert_audit_passed` invocation also requires audit exit `0` and
`Chain verification: PASSED`. The separately named order-independence regression passed
in the same logged four-test run (the attribution helper and LIVE-08 body at
executed-revision lines 30–64 and 484–521). These facts are assertion-backed consequences
of the logged `ok` results, not verbatim excerpts from the hashed log.

## Retained prior failed attempt

The previously canonical failed run was preserved rather than overwritten:

- Path: `51-LIVE-SCOPED-FAIL-PRIOR-9b78bd4f.log`.
- SHA-256: `9b78bd4f974ade0b74db03c09b133e60e3395e514d08a5ba73bc624988afebbc`.
- Result: exit `101`; this is failure evidence from commit `3e6e389`, before Plan 51-09 repaired the order-dependent test oracle. It is not evidence for the green result above.

## Full workspace gate

Exact command:

```sh
COMPOSE_VERIFY_CMD='cargo build --workspace && cargo test --workspace --no-fail-fast --features brokerd/mock-egress-ca' bash scripts/compose-verify.sh
```

- Completion time: `2026-08-08T16:44:49Z` (UTC).
- Exit status: `0`, preserved through a `bash -o pipefail` tee.
- Complete stdout/stderr: `51-LIVE-FULL.log`.
- SHA-256: `4bcb275b98dde637d7ac644a60227d33cc5ec47acf65e8898e2f1a4d4b34ee3e`.
- Observed result: all workspace unit, integration, acceptance, and doc-test binaries completed without failure; the retained output ends with `Composed Linux verification suite PASSED (Mailpit + mock GitHub).`
- The full suite included the Phase 50 coding/hold/planner regressions and the Linux `live_acceptance_v1_9_composed_success_chain`, which passed.
- Coverage caveat: `live_acceptance_v1_4_composed_three_legs` and
  `llm_planner_clean_allow_delivers` took their documented early-return paths because
  `OPENAI_API_KEY` was absent. Cargo suppresses their passing-test `SKIP` messages, but
  their `0.00s` durations in the retained full log and the guarded source branches show
  that these two LLM-dependent live legs were not exercised. They are green only in the
  sense that their test binaries completed without failure; this run must not be cited as
  passed live coverage for those two legs. No Phase 51 LIVE-07/LIVE-08 acceptance claim
  depends on them.
- The full-workspace command ran the two always-enabled tests in
  `live_acceptance_v1_10_cli`; it did not rerun the Linux fixture-gated LIVE-07/LIVE-08
  bodies because `live-proof-fixtures` is intentionally owned by the preceding scoped
  command. The scoped log is the authoritative LIVE-07/LIVE-08 execution evidence.
- `./scripts/check-invariants.sh` ran immediately after the compose command and reported `All invariant gates PASSED`; Gates 1, 2, 3, 4, 4b, and 6 passed, while Gate 5 reported its documented host-side `cargo not found` skip. The authoritative container build graph and complete workspace tests were green in the preceding compose command.

The invariant output was not retained as a separate hashed artifact. The preceding
invariant statement is executor-reported metadata, with the Gate 5 skip disclosed; it is
not independently reconstructible from the two compose logs.

Because both the scoped and full gates exited `0`, the Phase 51 validation map, broken-windows ledger, and LIVE-07/LIVE-08 requirement statuses were reconciled after these results were retained.
