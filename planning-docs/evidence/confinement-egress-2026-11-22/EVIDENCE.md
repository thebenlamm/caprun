# Evidence — confinement egress hardening (commit 103e1ab)

Design: `planning-docs/DESIGN-confinement-egress-hardening.md`.
All times UTC, read from the hosts' `date -u` (see the `*-meta.txt` files) or
AWS API timestamps.

## Hosts

| Run | Instance | Type / account | Launched (AWS) | Kernel | OS | Docker |
|-----|----------|----------------|----------------|--------|----|--------|
| 1 | `i-01ac51d4eae266e45` | c7i.4xlarge, project AWS account, tag `Project=caprun` | 2026-11-22T04:35:13Z | `7.0.0-1018-aws` | Ubuntu 24.04.5 LTS | 29.1.3 |
| 2 | `i-0e9e2864ac2341f8d` | c7i.4xlarge, project AWS account, tag `Project=caprun` | 2026-11-22T04:41:52Z | `7.0.0-1018-aws` | Ubuntu 24.04.5 LTS | 29.1.3 |

Both hosts used `instance-initiated-shutdown-behavior=terminate` and a
`shutdown -h` safety timer as the first boot command.

Source tarballs (`git archive`, verified by `sha256sum` on the host, recorded in meta):
- fix, commit `103e1ab9e04502f88717e3ce0fbfc91ed9fef632`: `1393d3cbd9da2426a1ff89498427ca82cc600fb8177a999d2e1b0af27084ba81`
- pre-fix baseline, commit `652c2b9`: `bbd59a76c9de49f061e446ea51219d7f814b02505ec46493d9678d2e52c5f59a`

## Results

Commands are the Phase 51 gates, unmodified, via `scripts/compose-verify.sh`
(Mailpit + mock GitHub). Full = `cargo build --workspace && cargo test --workspace
--no-fail-fast --features brokerd/mock-egress-ca`. Scoped =
`cargo test -p caprun --test live_acceptance_v1_10_cli --features
live-proof-fixtures,mock-egress-ca -- --test-threads=1`. Exit codes preserved
through `bash -o pipefail` + `tee`.

| Log | Commit | Toolchain (cargo, printed in log) | Exit | Passed / failed |
|-----|--------|-----------------------------------|------|-----------------|
| `run2-fix-full-rust1.97.log` | fix | 1.97.1 (`rust:1.97` retagged as `rust:1`) | **0** | **764 / 0** |
| `run2-fix-scoped-rust1.97.log` | fix | 1.97.1 | **0** | **8 / 0** |
| `run1-fix-full-rust1-floating.log` | fix | floating `rust:1` (version not printed) | 101 | 734 / 30 |
| `run1-fix-scoped-rust1-floating.log` | fix | floating `rust:1` | 0 | 8 / 0 |
| `run2-base652c2b9-full-rust1.100.log` | pre-fix | 1.100.0 (floating `rust:1` on 2026-11-22) | 101 | 720 / 31 |

`./scripts/check-invariants.sh` on run 2 (fix tree): `All invariant gates PASSED`
(recorded in `run2-meta.txt`, not a separate hashed artifact).

## Reading the failures (toolchain drift, not this change)

- Every one of run 1's 30 failures is `could not locate sibling binary
  caprun-exec-launcher`: on the newer cargo, test binaries live under
  `<target>/debug/build/<crate>/<hash>/out/`, beyond the 2-ancestor search in
  the launcher lookup. The same message appears 23 times in the pre-fix
  baseline log on cargo 1.100.0, so the lookup break predates this change.
  It is a pre-existing harness defect, NOT fixed here.
- The pre-fix baseline also fails 6 worker end-to-end tests (`substrate_demo`,
  `dag_chain_integrity`, `live_acceptance_confirm_path`, `live_acceptance_deny_path`,
  `s9_control_ab_taint_driven`, `s9_live_file_create_hostile_block`) with
  `tokio-rt-worker ... failed to allocate an alternative stack: Cannot allocate
  memory` / SIGABRT under the 512 MiB `RLIMIT_AS` — the multi-thread runtime
  the fix removes. All six pass with the fix in run 1 (floating `rust:1`,
  pulled ~7 minutes before run 2's baseline pull; its cargo version was not
  printed, so "same toolchain" is likely but unverified), and no
  `Cannot allocate memory` / `rt-worker` line appears in either fix log.
- `llm_planner_clean_allow_delivers` / `live_acceptance_v1_4_composed_three_legs`
  take their documented no-`OPENAI_API_KEY` early return in every run: the LLM
  path is NOT covered live. Locally, a dead-proxy run showed the confined worker
  receiving the sidecar's reply over the pre-connected stream (fail closed).

## Retained files (SHA-256)

```
60dc9e89849ec068830117e4e66f41f331ee11f1adcdfbb7b1cb5d7b6057f182  run2-fix-full-rust1.97.log
17ee34fca0b599e483d279a0af355c7e1cbf4cf6984bfd749813a908d3b98105  run2-fix-scoped-rust1.97.log
8e997dd958ac6f9f2c4a11c218cf0e4e1839a47156b2b0ed30a30494d109b794  run2-base652c2b9-full-rust1.100.log
5bc8c7a820ded25f0a5b3319a075884c2eba8669302e9a1563b0b2f33d8d2e19  run1-fix-full-rust1-floating.log
61bbc32d9acc7720c226b4d1e9f7c9140daa1b0df48b12a9027bdf51b5fd1c50  run1-fix-scoped-rust1-floating.log
6d65c1335cf2153dcc80c927227bbf09e5093f48601fed3348dc8171c8428ac7  run1-meta.txt
9e407bd96fb3a54d177498a00ef583849f96008793d64ed8a4547bea36024de4  run2-meta.txt
```

Boot logs are not retained in the repo (they contain presigned URLs).
