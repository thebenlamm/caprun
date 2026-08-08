# Phase 51 real-Linux LIVE evidence

## Environment and source identity

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

The passing `live_07_cli_multi_node_one_session_verify_chain` test observed exactly one surfaced Session, a successful `git_push_succeeded` event, a successful `github_pr_succeeded` event, and `caprun audit` output `Chain verification: PASSED`.

The passing sibling `live_08_cli_mid_loop_i2_block_genuine_taint` test observed exactly one Session and no `github_pr_succeeded` event. It selected exactly one durable `sink_blocked` anchor for sink `github.pr`, argument `body`, with non-empty taint, then proved `read_event_id == provenance_chain[0]` and that this ID names the `sink:process.exec:*` `process_exited` event. Its `caprun audit` check also observed `Chain verification: PASSED`. The order-independence regression test passed in the same run.

## Retained prior failed attempt

The previously canonical failed run was preserved rather than overwritten:

- Path: `51-LIVE-SCOPED-FAIL-PRIOR-9b78bd4f.log`.
- SHA-256: `9b78bd4f974ade0b74db03c09b133e60e3395e514d08a5ba73bc624988afebbc`.
- Result: exit `101`; this is failure evidence from commit `3e6e389`, before Plan 51-09 repaired the order-dependent test oracle. It is not evidence for the green result above.

## Full workspace gate

Pending. No full-workspace result or requirement/window completion is claimed in this section until Task 2 passes.
