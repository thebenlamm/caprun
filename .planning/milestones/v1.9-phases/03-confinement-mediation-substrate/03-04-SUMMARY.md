---
phase: 03-confinement-mediation-substrate
plan: "04"
subsystem: adapter-fs
tags: [scm-rights, fd-passing, sendmsg, recvmsg, nix, uds, capability-granting, cloexec]

dependencies:
  requires:
    - 03-01-SUMMARY  # adapter-fs crate scaffold with ENOSYS stubs
  provides:
    - pass_fd(socket_raw_fd, file_raw_fd) — SCM_RIGHTS sendmsg (broker side)
    - recv_fd(socket_raw_fd) — SCM_RIGHTS recvmsg + FD_CLOEXEC set (worker side)
    - protocol::{RequestFd, FdGranted} — serde IPC types (already complete from Plan 01)
    - Integration tests: round_trip + fd_cloexec passing on macOS (cross-platform)
  affects:
    - 03-05-PLAN  # caprun demo calls adapter_fs::pass_fd/recv_fd in the mediation loop

tech-stack:
  added:
    - nix uio feature (sendmsg/recvmsg ControlMessage/ControlMessageOwned now accessible)
  patterns:
    - cmsg_space!([RawFd; 1]) macro for correct platform-specific cmsg buffer sizing
    - BorrowedFd::borrow_raw(fd) to satisfy nix 0.31 AsFd bound on fcntl
    - Single ControlMessage::ScmRights slice per sendmsg (nix #464 guard)
    - FD_CLOEXEC set via fcntl immediately after recvmsg (defence-in-depth T-03-11)
    - ENODATA returned (not bogus fd) when no SCM_RIGHTS cmsg found (T-03-12)

key-files:
  created:
    - crates/adapter-fs/tests/fd_pass.rs (round_trip + fd_cloexec integration tests)
  modified:
    - crates/adapter-fs/src/lib.rs (pass_fd + recv_fd real implementation; unit test)
    - Cargo.toml (added uio to nix workspace feature set)

key-decisions:
  - "nix 0.31 uio feature required for sendmsg/recvmsg: the items are cfg-gated behind feature=uio in nix 0.31.3; added to workspace dep features."
  - "nix 0.31 fcntl takes AsFd not RawFd: used BorrowedFd::borrow_raw(fd) (unsafe, justified by valid fd postcondition of recvmsg) to satisfy the trait bound."
  - "No MSG_CMSG_CLOEXEC (Linux-only): fcntl after recv used instead for macOS compatibility, per RESEARCH.md Pattern 6."
  - "ENODATA returned for missing cmsg: surfaces truncated ancillary data rather than returning a bogus fd (T-03-12 mitigation)."

patterns-established:
  - "SCM_RIGHTS send: single ControlMessage::ScmRights(&[fd]) + 1-byte iov, sendmsg::<()>"
  - "SCM_RIGHTS recv: nix::cmsg_space!([RawFd; 1]) buffer, BorrowedFd::borrow_raw for fcntl"
  - "Cross-platform fd-pass tests: socketpair + temp file, no cfg gate (SCM_RIGHTS is POSIX)"

requirements-completed:
  - REQ-adapters-fs

coverage:
  - id: D1
    description: "pass_fd sends a file fd via SCM_RIGHTS over a Unix socket (broker side)"
    requirement: REQ-adapters-fs
    verification:
      - kind: unit
        ref: "crates/adapter-fs/src/lib.rs#pass_recv_fd_roundtrip"
        status: pass
      - kind: integration
        ref: "crates/adapter-fs/tests/fd_pass.rs#round_trip"
        status: pass
    human_judgment: false
  - id: D2
    description: "recv_fd receives the fd from SCM_RIGHTS, sets FD_CLOEXEC, returns Ok(RawFd)"
    requirement: REQ-adapters-fs
    verification:
      - kind: unit
        ref: "crates/adapter-fs/src/lib.rs#pass_recv_fd_roundtrip"
        status: pass
      - kind: integration
        ref: "crates/adapter-fs/tests/fd_pass.rs#fd_cloexec"
        status: pass
    human_judgment: false
  - id: D3
    description: "Worker reads file content via passed fd without ever calling open() on the path"
    requirement: REQ-adapters-fs
    verification:
      - kind: integration
        ref: "crates/adapter-fs/tests/fd_pass.rs#round_trip"
        status: pass
    human_judgment: false
  - id: D4
    description: "FD_CLOEXEC set on received fd immediately after recvmsg (T-03-11 mitigation)"
    requirement: REQ-adapters-fs
    verification:
      - kind: integration
        ref: "crates/adapter-fs/tests/fd_pass.rs#fd_cloexec"
        status: pass
    human_judgment: false

duration: 4min
completed: "2026-06-29"
status: complete
---

# Phase 3 Plan 4: adapter-fs SCM_RIGHTS Implementation Summary

**pass_fd/recv_fd via SCM_RIGHTS sendmsg/recvmsg: broker passes open file fds to the worker over UDS with FD_CLOEXEC set, proven by cross-platform socketpair round-trip and cloexec integration tests**

## Performance

- **Duration:** 4 min
- **Started:** 2026-06-29T21:15:04Z
- **Completed:** 2026-06-29T21:19:00Z
- **Tasks:** 2 (Task 1 TDD: RED + GREEN; Task 2: integration tests)
- **Files modified:** 3

## Accomplishments

- `pass_fd(socket_raw_fd, file_raw_fd) -> nix::Result<()>`: sends one fd in a single `ControlMessage::ScmRights` slice with a 1-byte iov payload via `sendmsg::<()>`
- `recv_fd(socket_raw_fd) -> nix::Result<RawFd>`: receives fd via `recvmsg` with a `cmsg_space!([RawFd; 1])` buffer, sets `FD_CLOEXEC` immediately via `fcntl`, returns `ENODATA` if no SCM_RIGHTS cmsg
- Integration tests in `fd_pass.rs`: `round_trip` proves the worker reads content via the passed fd only; `fd_cloexec` proves the CLOEXEC flag is set immediately after receive
- `cargo test -p adapter-fs`: 3 passed (1 unit + 2 integration), 0 failed — on macOS (cross-platform, no cfg gate)
- `cargo build --workspace`: clean

## Task Commits

1. **Task 1 RED** — `92a080e` (test: add failing lib test for pass_fd/recv_fd SCM_RIGHTS)
2. **Task 1 GREEN** — `89df487` (feat: implement pass_fd/recv_fd via SCM_RIGHTS sendmsg/recvmsg)
3. **Task 2** — `d8251e3` (test: add SCM_RIGHTS round-trip and cloexec integration tests)

## Files Created/Modified

- `crates/adapter-fs/src/lib.rs` — replaced ENOSYS stubs with real pass_fd/recv_fd + unit test
- `crates/adapter-fs/tests/fd_pass.rs` — replaced #[ignore] scaffolds with round_trip + fd_cloexec
- `Cargo.toml` — added `uio` feature to nix workspace dep

## Decisions Made

- `nix uio feature`: sendmsg/recvmsg/ControlMessage are cfg-gated behind `feature=uio` in nix 0.31.3; added to workspace dep to unlock them.
- `BorrowedFd::borrow_raw for fcntl`: nix 0.31's `fcntl` signature changed to `Fd: AsFd` (not `RawFd`). `BorrowedFd::borrow_raw(fd)` converts the received fd safely (marked unsafe; validity postcondition of recvmsg justifies it).
- `No MSG_CMSG_CLOEXEC`: that flag is Linux-only. Using fcntl after recv (per Pattern 6) keeps the implementation cross-platform so the round-trip tests run on the macOS dev machine.
- `ENODATA for missing cmsg`: avoids returning a bogus fd if ancillary data was dropped/truncated (T-03-12 mitigation).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] nix uio feature missing — sendmsg/recvmsg/ControlMessage gated**
- **Found during:** Task 1 GREEN (first build attempt)
- **Issue:** nix 0.31.3 gates `sendmsg`, `recvmsg`, `ControlMessage`, and `ControlMessageOwned` behind `feature = "uio"`. The workspace dep only listed `["fs", "socket", "resource", "process", "signal"]`.
- **Fix:** Added `"uio"` to the nix features array in root `Cargo.toml`.
- **Files modified:** `Cargo.toml`
- **Committed in:** `89df487`

**2. [Rule 3 - Blocking] nix 0.31 fcntl takes AsFd, not RawFd**
- **Found during:** Task 1 GREEN (same build)
- **Issue:** `nix::fcntl::fcntl` signature in 0.31 is `pub fn fcntl<Fd: AsFd>(fd: Fd, ...)`. Passing `RawFd` (i32) directly fails to satisfy the trait bound.
- **Fix:** Used `unsafe { BorrowedFd::borrow_raw(fd) }` to produce a `BorrowedFd<'_>` which implements `AsFd`. The `unsafe` is justified because `fd` was just returned from `recvmsg` (valid fd postcondition).
- **Files modified:** `crates/adapter-fs/src/lib.rs`
- **Committed in:** `89df487`

---

**Total deviations:** 2 auto-fixed (both Rule 3 — blocking API mismatches in nix 0.31)
**Impact on plan:** Both fixes were necessary for compilation. No scope creep. Pattern 6 from RESEARCH.md implemented verbatim once the API surface was corrected.

## Threat Surface Scan

No new surface beyond the plan's threat register. The mitigations stated in T-03-11 and T-03-12 are fully implemented:

| Threat | Mitigation Implemented |
|--------|----------------------|
| T-03-11: fd leaks into grandchild exec | FD_CLOEXEC set immediately in recv_fd; proven by fd_cloexec integration test |
| T-03-12: truncated ancillary data returns bogus fd | ENODATA returned if no SCM_RIGHTS cmsg found (not a 0 or -1 fd) |
| T-03-13: worker re-opens path | Worker has no open() capability in production (Landlock — Plan 02); proven by Plan 05 demo |

## Known Stubs

None — all stubs from Plan 01 replaced with real implementations. protocol.rs (RequestFd, FdGranted) was already complete from Plan 01.

## Self-Check: PASSED

| Check | Result |
|-------|--------|
| crates/adapter-fs/src/lib.rs (pass_fd real impl) | FOUND |
| crates/adapter-fs/tests/fd_pass.rs (round_trip + fd_cloexec) | FOUND |
| Cargo.toml nix uio feature | FOUND |
| Commit 92a080e (TDD RED) | FOUND |
| Commit 89df487 (TDD GREEN) | FOUND |
| Commit d8251e3 (integration tests) | FOUND |
| cargo test -p adapter-fs: 3 passed, 0 failed | PASSED |
| cargo build --workspace: clean | PASSED |

---
*Phase: 03-confinement-mediation-substrate*
*Completed: 2026-06-29*
