# DECISION — Defer `git.push` (GIT-02/GIT-03) from v1.8 to v1.9

**Date:** 2026-07-18
**Decider:** orchestrator (autonomous v1.8 run; Ben asleep — FLAGGED FOR REVIEW)
**Status:** Decided — git.push deferred; v1.8 ships git.commit + http.request + github.pr.
**Authority:** `planning-docs/DESIGN-git-github-http-sinks.md` §2.1 / §11 Open-Item-1 HARD CONSTRAINT,
cleared at the Phase-35 design gate (`DESIGN-GATE-RECORD-v1.8.md`, BLOCKER-1). This decision is the
gate's own pre-authorized fail-safe, not an ad-hoc descope.

## The constraint (from the cleared design gate)

BLOCKER-1 (fresh adversarial code-trace, Phase 35): a `git.push` confined child that reaches the
network **cannot** have its destination pinned by seccomp — seccomp-bpf filters syscall
numbers/scalar registers, and `connect()`'s destination is a `struct sockaddr *` behind a pointer it
cannot dereference; Landlock network rules need kernel 6.7 (> the 5.13 floor) and filter by port only.
The only seccomp "relaxation" possible is all-or-nothing `AF_INET` allow = **arbitrary egress** from a
child holding a live push credential — the exact exfiltration primitive the taint model exists to defeat.

The gate therefore re-decided FORK-1 to: **the git.push child stays fully net-denied; the destination
pin is broker-mediated (application-layer resolve-and-pin)**, with the HARD CONSTRAINT:

> "the destination pin MUST be enforced by a broker/netfilter layer that can actually see the
> destination — it may NEVER be claimed of seccomp — and if no fully-unprivileged destination-pinning
> mechanism proves feasible, `git.push` is DEFERRED to a later milestone rather than shipped with
> arbitrary child egress."

## Why defer now (feasibility assessment)

A sound, fully-unprivileged (no `--privileged`, no root, kernel ≥5.13), destination-pinned git.push
egress requires one of:
1. **pasta/slirp4netns + per-push netns egress filter** — real unprivileged user-mode networking with
   address restriction, but adds an external binary dependency and non-trivial integration.
2. **Broker-proxied git smart-HTTP transport** — the broker terminates/pins the network leg while the
   child generates the pack; requires implementing/proxying git's HTTP(S) transport in the TCB.
3. **SCM_RIGHTS pre-connected fd + git `ext::` transport** — most caprun-native (reuses the fd-passing
   muscle, child stays fully net-denied) but wiring git's authenticated HTTPS transport onto a
   broker-supplied fd (TLS termination boundary) is substantial.

Each is a **genuinely new trust posture and/or a new dependency** that the gate itself flagged as "the
riskiest surface in the project to date," warranting its own design iteration + fresh adversarial
review (a mini design-gate). That is not something to design, adversarially review, implement into the
security TCB, and live-prove correctly in a single unattended autonomous session without Ben's input on
the trust-posture/dependency choice. Shipping a rushed or unsound version would reintroduce exactly the
BLOCKER-1 exfil primitive. Deferral is the integrity-preserving outcome the gate anticipated.

## What v1.8 ships instead (unchanged, sound)

- **git.commit** (GIT-01) — Pattern B confined child, MutateReversible, config/hooks neutralized,
  Linux-verified (3/3 spawn tests) incl. the debug-fixed Landlock `/dev` + workspace-write rights and
  the exit-code failure handling.
- **http.request** GET (HTTP-01/02/03) — Pattern A egress, new `mint_from_http` inbound taint + I1
  demotion, SSRF resolve-and-pin, adversarial-cleared + hardened (aws-lc-rs removed, port pin, timeout,
  body cap, failed-GET audit event).
- **github.pr** (GITHUB-01/02/03/04) — bearer token (broker-only, opaque audit), session auth-grant,
  tainted title/body Block, duplicate-PR CAS, confirm-release P33/P34 — adversarial VERDICT APPROVE.
- **Composed live proof (Phase 40, rescoped):** exec → fs (edit) → git.commit → github.pr (mock
  endpoint) + an http.request GET leg, every step gated/tainted/audit-DAG-chained on real Linux, with
  the adversarial attack legs (tainted PR-body, tainted GET url/SSRF, tainted commit message) each
  deterministically Blocked. The real-push leg is the one documented gap (mock accepts the PR head).

## Moved to v1.9 (Effect Breadth II continued)

- **GIT-02** — `git.push` sink (broker-mediated destination-pinned egress; opens with a dedicated
  design-gate for the unprivileged egress mechanism + fresh adversarial review).
- **GIT-03** — tainted push remote/refspec Block + confirm-release for git.push.
- The v1.8 DESIGN doc's §2 (git.push model), §2.7 (payload-at-confirm), §2.5 (captured-output scrub)
  carry forward as the starting design for v1.9's git.push (the model is pinned; only the
  fully-unprivileged egress realization is open).

## Honesty note for the milestone record

v1.8 delivers 3 of the 4 originally-scoped sinks. The Safe Coding Agent anchor is demonstrated for
edit→commit→open-PR; the push step (commit→remote) is deferred. This is disclosed in PROJECT.md and the
milestone audit — not papered over.
