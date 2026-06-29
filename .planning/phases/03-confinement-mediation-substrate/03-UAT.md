---
status: testing
phase: 03-confinement-mediation-substrate
source: [03-VERIFICATION.md]
started: 2026-06-29T22:00:00Z
updated: 2026-06-29T22:00:00Z
---

## Current Test

number: 1
name: Confined worker cannot read ~/.ssh/id_rsa (Landlock deny-all)
expected: |
  On Linux (ubuntu >= 22.04, kernel >= 5.13):
  `cargo test -p sandbox --test confinement_integration` runs negative_fs;
  confine-probe exits 0 (open of ~/.ssh/id_rsa returns EACCES).
awaiting: user response

## Tests

All 8 tests below are `#[cfg(target_os="linux")]` and require ONE Linux run.
Recommended single command on Linux: `cargo test --workspace`
(ubuntu >= 22.04, kernel >= 5.13 for Landlock). No --privileged needed —
the confinement stack is fully unprivileged. If Docker's default seccomp
profile blocks landlock()/seccomp(), add `--security-opt seccomp=unconfined`.

### 1. negative_fs — confined worker cannot read ~/.ssh/id_rsa
expected: confine-probe exits 0 (Landlock deny-all → open returns EACCES)
result: [pending]

### 2. negative_net — confined worker cannot open a TCP socket
expected: confine-probe exits 0 (seccomp denies AF_INET/AF_INET6 → EPERM)
result: [pending]

### 3. negative_exec — confined worker cannot exec an un-allowlisted binary
expected: confine-probe exits 0 (seccomp denies execve → EPERM)
result: [pending]

### 4. apply_rlimits enforcement (RLIMIT_AS + RLIMIT_CPU)
expected: rlimits applied; oversized alloc / CPU overrun is bounded as designed
result: [pending]

### 5. server_accept — broker serves abstract-namespace UDS IPC
expected: broker binds `\0/agentos/<session_id>`, accepts a framed connection
result: [pending]

### 6. create_session_round_trip — broker CreateSession over UDS
expected: CreateSession → Session row in SQLite + SessionCreated reply
result: [pending]

### 7. substrate_demo — end-to-end no-LLM mediation (caprun)
expected: caprun exits 0; confined worker reads file via passed fd (not open());
  file_read Event present in audit DAG with bytes_read == content length
result: [pending]

### 8. dag_chain_integrity — audit DAG chain unbroken
expected: verify_chain == true; exactly 3 events in causal order with linked
  parent_hashes: session_created → fd_granted → file_read
result: [pending]

## Summary

total: 8
passed: 0
issues: 0
pending: 8
skipped: 0
blocked: 0

## Gaps
