# caprun post-mortem (archived 2026-11-22)

caprun set out to answer one question: can an AI agent run on stock Linux with
no ambient authority, so that an attacker who controls the data it reads still
cannot steer a sensitive action? The design had two halves: a kernel-confined
worker whose only way out is the broker, and value-level taint that blocks any
attacker-derived value from a sensitive sink argument unless a human confirms
the literal value (invariant I2).

The project is archived. This document records what held, what broke, and what
was never tested, so the design can be reused without inheriting its gaps.

## What was built

- About 59k lines of Rust across `runtime-core`, `sandbox`, `brokerd`,
  `executor`, `adapter-fs`, `llm-planner` and `cli/`. About 26.5k of those are
  in `tests/` directories. Outside them, about 19.7k lines are neither blank
  nor comments, and that count still includes inline unit tests.
- Nine sinks behind the broker: email, file create/write, process exec,
  git commit, git push, GitHub PR, HTTP read, HTTP write.
- A hash-chained SQLite audit log in which every block is a provable edge from
  the raw read event to the sink argument.
- Three intents, driven by a deterministic planner. The optional LLM planner
  chooses arguments for one step of the two single-step intents only.

## What held: the taint model

An independent code review on 2026-11-22 traced the I2 path and found it
sound by construction:

- The planner and worker handle only opaque value handles (`PlanArg { name,
  value_id }` in `crates/runtime-core/src/plan_node.rs`), never literals.
- The broker mints every value. `mint_from_intent` is the only place a
  `UserTrusted` value is created (`crates/brokerd/src/quarantine.rs`). Derived
  values take the union of their inputs' taint, always add `WorkerExtracted`,
  and drop `UserTrusted`.
- Output from process exec and HTTP is tainted when it arrives.
- The executor rejects unknown sinks and arguments, then blocks any untrusted
  value in a sensitive argument.

One weakness is noted for any reuse. Trusted values come only from the
`ProvideIntent` message, which the worker itself sends. Ordering enforces this
(once, before any file is opened), but its origin does not. That is fine for a
hardcoded worker. It would be weak for a worker driven by an LLM.

## What broke: the confinement half

The 2026-11-22 review found the kernel confinement weaker than claimed. Items
1–4 were reproduced on Linux 6.8 with an out-of-repo probe that calls the real
`sandbox::apply_confinement()`. Items 5–6 were found by code review; no
old-kernel exploit was run. All six are fixed in commit `103e1ab` and covered
by tests that fail on the pre-fix code. The reproductions are described in
`planning-docs/DESIGN-confinement-egress-hardening.md` (§1, §6). The
post-fix regression runs and a pre-fix baseline are in
`planning-docs/evidence/confinement-egress-2026-11-22/`.

1. **Only one thread was confined.** Landlock and seccomp apply to the calling
   thread only. The worker ran tokio's multi-thread runtime, whose four worker
   threads started before confinement and stayed fully unconfined, with
   network and filesystem access, in the same address space.
   *Fix:* single-threaded runtime, plus a guard that refuses to confine a
   process with more than one thread.
2. **io_uring bypassed the network filter.** The seccomp filter was a denylist
   on the `socket` syscall. `IORING_OP_SOCKET` creates a socket without calling
   it, and `connect()` on that socket reached the network.
   *Fix:* deny `io_uring_setup`, `io_uring_enter` and `io_uring_register`.
3. **AF_UNIX was a full escape.** Only AF_INET and AF_INET6 were denied. A
   process-exec child reached the user's D-Bus session bus, and
   `systemd-run --user` ran an arbitrary command outside the sandbox.
   *Fix:* deny `socket()` for every family. The LLM-planner connection is now
   opened before the worker confines itself.
4. **Other socket families were open** (AF_NETLINK and others). Closed by the
   same fix.
5. **Landlock failed open.** On a kernel without Landlock, the status was
   printed and the worker ran with no filesystem confinement.
   *Fix:* refuse to run when Landlock is not enforced.
6. **Path truncation and terminal injection.** Landlock's Truncate right needs
   kernel 6.2, not 5.19 as the code comments said, so on older kernels within
   the supported 5.13 floor `truncate(path)` was unchecked. `ioctl(TIOCSTI)`
   could inject keystrokes into the inherited terminal on older kernels.
   *Fix:* seccomp denies both.

### Why the tests didn't catch them

- The confinement probe is a single-threaded binary, so it could not see the
  per-thread bug.
- The network test used `socket(AF_INET)` only. It checked the rule as written,
  not other ways of reaching the network.
- The plan (`planning-docs/PLAN.md`) lists namespaces as part of the
  sandbox, but a phase research note downgraded the network namespace to
  "optional but stronger", and it was never built. Unprivileged user
  namespaces are blocked on Ubuntu 24.04, so it was not available without
  privilege, and nothing replaced it.
- The `kernel ≥ 5.13` floor was stated but never tested on a kernel that old.

## Lessons for anyone reusing the design

- **Confine before any threads exist, and check that it happened.** Read
  `/proc/self/task` before confining. Under a multi-thread runtime, put
  confinement before the runtime starts.
- **A seccomp denylist is a list of the bypasses you already know about.**
  If you can't afford an allowlist, deny the capability itself (all socket
  creation, io_uring), not one way of reaching it.
- **AF_UNIX is not local-only in practice.** Session buses and container
  sockets turn any reachable Unix socket into code execution.
- **Make confinement fail closed.** A status line in the log is not a check.
- **Test escapes from the attacker's side:** other threads, other syscalls,
  other socket families. Testing that the stated rule works is not enough.

## What was never tested

- **A multi-step LLM agent in the loop.** The central product claim was that a
  tool-using LLM agent can do useful work without a human confirming every
  effect once it has read untrusted data. That was never measured. Taint marks
  every value derived from a read file or exec output, so in a real coding loop
  most effects may need confirmation. This is the open question for the
  approach.
- **The LLM planner after the confinement fix.** A real OpenAI-backed
  planner ran and passed on single-step intents in earlier phases (21, 22,
  40), before commit `103e1ab`. In the 2026-11-22 verification runs those
  tests skipped because no API key was present. After the fix, the LLM path
  was checked only for transport, with a dummy key and a dead proxy.
- **Outside users.** There are no outside commits, issues, stars or forks.

## Remaining known gaps (not fixed)

- The seccomp filter is still a denylist, not an allowlist. An unknown
  syscall that creates a socket would bypass it.
- x32 syscall numbers are not filtered. This only matters on kernels built
  with `CONFIG_X86_X32_ABI`.
- The planner sidecar accepts any number of connections.
- There is no assertion on the worker's inherited file descriptors.
- The per-primitive public sandbox functions don't check the thread count
  themselves. Only `apply_confinement()` and the exec launcher do.
- On cargo 1.100.0, test binaries move under `<target>/debug/build/…/out/`
  and the exec-launcher sibling lookup fails (1.98 and 1.99 untested). Pin the
  toolchain to 1.97.1 (the verified version) or fix the lookup.
