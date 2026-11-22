# DESIGN — Confinement egress hardening (io_uring / socket-family / Landlock fail-open)

Status: **PROPOSED, revised after adversarial design review** (2026-11-22).
Pre-implementation design gate for a sandbox (security-boundary) change. The
first draft was REJECTED by a fresh non-self adversarial review (1 BLOCKER,
2 MAJOR, 3 MINOR); §6 records each finding and its disposition. Linux
regression on the authoritative EC2 harness is still pending.

## 1. Defect (reproduced 2026-11-22, Linux 6.8, `io_uring_disabled=0`)

A process confined by the shipped `sandbox::apply_confinement()` can still reach
the network and arbitrary local services. Probe (out-of-repo scratch crate that
calls the real `apply_confinement()`), observed output:

```
[sandbox] Landlock status: FullyEnforced
std TcpStream::connect -> Err(EPERM)                 # seccomp socket(AF_INET) rule works
open /etc/passwd -> Err(EACCES)                      # Landlock works
UnixStream::connect(outside path) -> Ok(())          # D2
io_uring IORING_OP_SOCKET(AF_INET) -> 6              # D1
connect() on uring socket to 127.0.0.1:<port> -> 0   # D1
listener ACCEPTED from 127.0.0.1:<port>              # D1
```

Root causes (cited):
- **D1 — io_uring bypasses the socket() filter.** `apply_worker_filter`
  (`crates/sandbox/src/seccomp.rs:62-98`) is a denylist (mismatch action
  `Allow`, line 97) that matches the `socket` *syscall number*.
  `IORING_OP_SOCKET` (kernel ≥5.19) creates the socket inside the kernel without
  the `socket` syscall, so the filter never sees it. `connect()` on the resulting
  fd is allowed. `exec_child_filter` (line ~143-193) has the same shape.
- **D2 — AF_UNIX is unconfined.** Only `AF_INET`/`AF_INET6` are denied. Landlock
  ABI V3 (`landlock.rs:20,71`) does not mediate UDS connect. A confined worker can
  connect to any reachable pathname socket (D-Bus, ssh-agent, docker.sock) or
  abstract socket. Previously noted, unresolved: `DESIGN-session-trust-coherence.md:30`.
- **D3 — other socket families are open.** `AF_NETLINK`, `AF_PACKET`
  (cap-gated), `AF_VSOCK`, `AF_ALG`, etc. are not denied (denylist).
- **D4 — Landlock fails open.** `deny_all_filesystem` / `exec_child_ruleset` use
  the crate default (best-effort) and only `eprintln!` the status
  (`landlock.rs:30`). On a kernel without Landlock the worker runs with no
  filesystem confinement and does not fail.

Prior-art search (2026-11-22): `git grep -il` over `*.md *.rs *.sh` for
`io_uring` (0 hits), `AF_UNIX`, `unix socket`, `CLONE_NEWNET`, `network namespace`,
`netns`, `seccomp allowlist`, `default-deny`, `HardRequirement` (0),
`BestEffort` (0), `FullyEnforced`, `PartiallyEnforced` (0), `ABI V4 /
LANDLOCK_ACCESS_NET`. No documented decision accepts D1/D3/D4. D2 is
documented as a known reachability fact, not as an accepted risk. netns
appears only as an option for git.push egress (DECISION-git-push-deferral-v1.8.md:31),
rejected there for needing new privilege posture.

## 2. Rejected: network namespace

Unprivileged user namespaces are blocked on the target platform: Ubuntu 24.04
ships `kernel.apparmor_restrict_unprivileged_userns=1`; on this host
`unshare -Urn true` fails (`write /proc/self/uid_map: Operation not permitted`).
The confinement stack must stay fully unprivileged (CLAUDE.md, PLAN.md), so a
netns cannot be the control. Also: abstract UDS names are per-netns, which would
break the post-confinement sidecar connect unless pre-connected (see §3.2).

## 3. Proposed fix

### 3.1 Worker seccomp (`apply_worker_filter`)
Keep the existing execve/execveat denies. Replace the AF_INET/AF_INET6 socket
rule with:
- **`socket` — deny unconditionally** (all families, including AF_UNIX).
- **`socketpair` — allow** (creates only an unnamed connected pair; no egress).
- **`io_uring_setup`, `io_uring_enter`, `io_uring_register` — deny** (closes D1
  and every other io_uring-mediated syscall-filter bypass, e.g. IORING_OP_OPENAT
  is already covered by Landlock, IORING_OP_CONNECT on an inherited fd, etc.).

The worker's only sockets are then the ones it holds at confinement time (the
broker stream) plus fds the broker passes via SCM_RIGHTS. This closes D1, D2, D3
for the worker by construction, without needing to filter sockaddrs.

Why not a full syscall allowlist: the worker runs a tokio runtime + std +
serde; an allowlist must be derived empirically and any missed syscall kills
the worker on a path only exercised on EC2 (LLM planner, confirm hold). The
three-syscall-class deny above removes every *known* egress primitive
(socket creation + io_uring). Residual: an unknown future socket-creating
syscall. Recorded as residual R1; an allowlist remains a valid follow-up.

### 3.2 Pre-connect the LLM planner sidecar
`LlmPlanner::plan()` currently connects to the sidecar's abstract socket
**after** confinement (`cli/caprun/src/worker.rs:374-381`,
`cli/caprun/src/planner.rs:533,940`). Under 3.1 that `socket()` would fail.
Change: when `CAPRUN_PLANNER=llm` and `PLANNER_SOCK` is set, the worker connects
to the sidecar **before** `apply_confinement()` (same bounded retry), and passes
the stream into `LlmPlanner`. `LlmPlanner` holds `Mutex<Option<UnixStream>>`;
the first request takes it; any second request fails closed (exit 1, no node
submitted) — today the LLM path is single-node only (`worker.rs` bails on
SafeCodingWorkflow), so exactly one request is made.

### 3.3 Exec-child seccomp (`exec_child_filter`)
Same shared deny set as the worker (one `install_filter`, so the two cannot
drift), minus execve/execveat:
- `socket` — **deny unconditionally** (all families). *Revised:* the first draft
  kept AF_UNIX for the child; review BLOCKER B1 showed that is a full escape
  (§6). `socketpair` stays allowed.
- `io_uring_setup/enter/register` — deny.
- Cost: a target command can no longer create any socket (e.g. a test suite
  that binds a local UDS server). Network was already denied; local IPC
  servers are the new loss.

### 3.3a Shared additions (both filters) from review
- `truncate(path)` — deny (M1). `ftruncate` stays allowed.
- `ioctl(_, TIOCSTI | TIOCLINUX, ..)` — deny (M2).

### 3.4 Landlock fail-closed (D4)
Both `deny_all_filesystem` and `exec_child_ruleset` return `Err` if
`status.ruleset == RulesetStatus::NotEnforced`. `PartiallyEnforced` is accepted:
kernels 5.13–6.1 lack V2 `Refer` (5.19) and/or V3 `Truncate` (**6.2**, corrected
from the first draft's "5.19"). Unhandled `Truncate` IS an integrity gap
(review M1: `truncate(victim,0)` succeeded when only V2 rights were handled), so
seccomp now denies `truncate(path)` as the backstop (§3.3a). Unhandled `Refer`
only matters with write access to two directories, which the worker has none of
and the exec child has only inside the workspace.
Callers already propagate `Err` → worker exits / launcher `expect` panics (fail
closed).

### 3.5 Optional, same change: Landlock ABI V4 TCP rules
Not proposed. Redundant with 3.1 for the worker; for the exec child it would
only add defense-in-depth on ≥6.7 kernels. Keep the diff small.

## 4. Tests (RED before GREEN)

**As implemented** (all observed FAILING on the pre-fix sandbox source and
PASSING after, 2026-11-22, Linux 6.8; note `cargo test -p sandbox` does NOT
rebuild `caprun-exec-launcher` — rebuild it before trusting exec-child results):
worker probe ops `uring`, `netlink`, `unix <path>`, `truncate`, `tiocsti` →
`negative_io_uring`, `negative_netlink_socket`, `negative_unix_socket`,
`negative_path_truncate`, `negative_tiocsti`; exec child
`exec_child_io_uring_denied`, `exec_child_netlink_denied`,
`exec_child_unix_socket_denied`, `exec_child_path_truncate_denied_ftruncate_allowed`;
benign `exec_child_unix_socketpair_allowed` passes before and after.
The `truncate`/`tiocsti` probes use a nonexistent path / a pipe so the kernel's
own answer (ENOENT / ENOTTY) differs from seccomp's EPERM — otherwise Landlock
V3 on this kernel would make them pass pre-fix.

Original plan:

`crates/sandbox/src/bin/confine-probe.rs` gains ops; `confinement_integration.rs`
gains:
- `negative_io_uring_socket` — after `apply_confinement()`, `io_uring_setup`
  returns EPERM (and no AF_INET socket is obtained).
- `negative_unix_socket` — `UnixStream::connect` to a pathname socket outside
  the workspace fails EPERM.
- `negative_netlink_socket` — `socket(AF_NETLINK, …)` fails EPERM.
Exec-child (`exec_child_confinement.rs`): io_uring_setup denied; AF_NETLINK
denied; AF_UNIX socketpair still works (benign path).
Each new negative test must be observed FAILING on the pre-fix code first.

Regression: worker e2e + LLM-planner sidecar tests run locally (Linux 6.8, no
Docker) where possible; full `mailpit-verify.sh`/`compose-verify.sh` on EC2.

## 5. Invariant impact
None weakened. No new crates (`seccompiler` already supports `Ne`/unconditional
rules). No change to executor, taint, or the plan-node path.

## 6. Adversarial design review — findings and disposition

| # | Finding | Disposition |
|---|---------|-------------|
| B1 | Exec child with AF_UNIX allowed reaches the user D-Bus bus (`srw-rw-rw-`); `systemd-run --user` runs an unconfined command — full escape. Lead re-verified on shipped code: `touch` outside the sandbox via systemd-run succeeded while a direct `touch` was denied. | **Fixed** (§3.3): socket() denied for all families in the exec child. Re-run after fix: `Failed to connect to bus: Operation not permitted`, no file. |
| M1 | Truncate is ABI V3 = kernel 6.2, not 5.19; PartiallyEnforced leaves `truncate(path)` unchecked. | **Fixed**: seccomp denies `truncate`; comments corrected. |
| M2 | TIOCSTI on the inherited terminal (kernels < 6.2 / legacy_tiocsti). | **Fixed**: ioctl TIOCSTI/TIOCLINUX denied. |
| m1 | x32 syscall numbers (`__X32_SYSCALL_BIT`) are not filtered by seccompiler. | **Residual R3, disclosed.** Not reachable on kernels built without `CONFIG_X86_X32_ABI` (this host: ENOSYS). Not testable here. |
| m2 | Planner sidecar accepts unlimited connections. | **Residual R4, disclosed.** After §3.1/§3.3 neither confined process can create a socket; remaining reachers are same-user unconfined processes, outside the threat model. |
| m3 | No assertion on the worker's inherited fd set. | **Residual R5, disclosed.** |

### Independent diff review (post-implementation)

| # | Finding | Disposition |
|---|---------|-------------|
| T1 BLOCKER (pre-existing) | Landlock `restrict_self` and seccompiler `apply_filter` confine only the CALLING thread. The worker used the default multi-thread tokio runtime, so 4 `tokio-rt-worker` threads stayed unconfined (`Seccomp: 0`); a task on one opened an AF_INET socket and read `/etc/hostname`. Lead reproduced. | **Fixed**: worker uses `#[tokio::main(flavor = "current_thread")]`; `sandbox::require_single_thread()` (reads `/proc/self/task`, before Landlock) makes `apply_confinement()` and the exec launcher fail closed with >1 thread. Test `negative_multithreaded_confinement_refused` RED without the guard, GREEN with it. Reviewer re-traced the real worker binary: 1 thread, `Seccomp: 2`. |
| minors | Stale docs/comments; untested Landlock fail-closed branch; ioctl-overwrite footgun in `install_filter`. | **Fixed** (doc edits, `require_enforced` unit tests, `install_filter` refuses `SYS_ioctl` as an extra). |
| R6 MINOR | The per-primitive public fns (`apply_worker_filter`, `exec_child_filter`, `deny_all_filesystem`, `exec_child_ruleset`) don't themselves check the thread count. | **Residual, disclosed.** Only the launcher calls them directly, after `require_single_thread`. |

Residuals: R1 (denylist, not allowlist: an unknown future socket-creating
syscall would bypass), R3, R4, R5, R6. No network namespace (§2).
io_uring tests accept EPERM, which a host with `kernel.io_uring_disabled=2`
would also return without seccomp; the RED evidence above is from a host with
`io_uring_disabled=0`.
