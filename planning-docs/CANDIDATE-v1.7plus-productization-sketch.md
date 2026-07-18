# CANDIDATE — caprun productization sketch (v1.7+)

> **STATUS: DISCUSSION SKETCH, NOT A COMMITTED PLAN.** This is not a GSD milestone,
> not roadmapped, not gated. It exists to answer "what's between the current proven
> security core and something a person can actually use?" Nothing here overrides
> `planning-docs/PLAN.md` or `.planning/ROADMAP.md`. Do not GSD-ingest as committed scope.
> Author: orchestrator, mid-Phase-28. Confirm the anchor use case (§1) before any of this is real.

---

## 0. Where we are (baseline, honest)

**What exists = a proven security core, not a product.** Through v1.5 (shipped) and v1.6
(in progress) caprun proves the hard, differentiating claim: a kernel-confined worker with
**no ambient authority**, whose only egress is broker-mediated plan nodes, where a genuinely
propagated taint chain **deterministically blocks value-injection at a sink (I2)**, with an
authenticated audit DAG.

Concretely present today:
- Confinement boundary: namespaces, Landlock, seccomp-bpf, `no_new_privs`, rlimits, default-deny net.
- Broker reference monitor: Session lifecycle, SQLite audit DAG (now keyed/HMAC after Phase 28), UDS IPC.
- Deterministic executor enforcing I2 (the security differentiator) + slot-type binding (v1.5).
- **Two real sinks only:** `email.send` (live SMTP via lettre) and `file.create`.
- Planner: a stub/deterministic planner + one adversarial-LLM planner experiment (v1.4 sidecar seam).
- Session trust state (I0/I1) + single-shot human confirmation.

**It is Linux-only (kernel ≥5.13), single-host, single-session, CLI/test-harness grade.**

---

## 1. The load-bearing decision: anchor use case  ⟵ CONFIRM THIS FIRST

Everything below assumes ONE concrete workflow to make usable. My recommendation:

**★ ANCHOR A — "Safe coding agent."** An LLM agent that can read/edit a repo, run commands
(build/test) in the sandbox, make allowlisted HTTP calls, and open a GitHub PR — where any
irreversible/external effect (the PR, an outbound call) is policy-gated and value-injection
is blocked. Why this one:
- Closest to the existing substrate (`file.create` already exists; GitHub is already named as
  future scope in CLAUDE.md; default-deny net is already the base for authorized HTTP).
- Hot, fundable, timely pain: teams are actively scared to let coding agents run loose in
  repos/CI (secret exfiltration, rogue pushes). caprun's pitch — "the agent *structurally cannot*
  exfiltrate your secrets or push unreviewed" — sells itself.
- Produces a demo that closes: *watch an agent get prompt-injected into trying to leak a secret
  via a PR, and get deterministically blocked, with the audit DAG proving why.*

Alternatives (pick instead if they fit your GTM better):
- **B — Safe data/ops agent:** query DB + call internal APIs + write reports; no destructive
  write without confirm. (Enterprise ops buyer.)
- **C — Safe integration/automation runner:** a policy-boxed replacement for "an agent with an
  API key and a shell." Broader, vaguer, harder to demo.
- (Explicitly NOT desktop automation — CLAUDE.md rules caprun out of that.)

---

## 2. What a usable slice of Anchor A actually needs

Grouped by subsystem. Each new sink is **real work** because it must go through the
plan-node → taint → executor(I2) → audit path — that discipline *is* the product, and also
what makes "just add a sink" non-trivial.

| # | Capability | Why it's needed | Rough effort |
|---|-----------|-----------------|--------------|
| S1 | `process.exec` sink — run a command in the sandbox, capture + **taint** stdout/stderr | The core primitive of a coding agent (build, test, lint). Also the most powerful/dangerous sink. | **High** |
| S2 | Filesystem read/write breadth (beyond `file.create`) — read many repo files, write edits | Agent must see and modify the repo. adapter-fs (fd passing) is the seam. | Medium |
| S3 | `http.request` sink — outbound HTTP to an **allowlisted host** | Fetch docs, call APIs. Punches authorized holes in default-deny net via broker. | Medium |
| S4 | `git` / `github.pr` sink — commit + open a PR (the irreversible external effect) | The "important action" that triggers I2 / human confirmation. Named v1.7 scope already. | Medium-High |
| P1 | Real LLM planner loop — multi-step, tool-use, retries, error handling (Claude) | Replaces the stub; turns "arbitrary intent" into a plan-node stream. Build on the v1.4 sidecar seam (planner in a separate unconfined process; worker submits plan nodes). | **High** |
| Pol1 | Minimal declarative policy — per-session sink allowlist + arg constraints (which hosts, paths, repos) | "Which session may call which sink with which args." Cedar is the eventual engine; start with a hardcoded-schema policy file (manual ops before framework). | Medium |
| U1 | Thin SDK/CLI + audit-DAG viewer — define intent, point at workspace, run, inspect the proof | The front of the product. No web UI yet. The audit view IS the trust surface. | Medium |
| D1 | Packaging — single binary + documented "run on a Linux box/container" path | So a partner can actually install it. Still Linux-only. | Low-Medium |

Deliberately **excluded** from the first usable slice (defer until pulled): Cedar policy engine,
cross-host delegation / Biscuit crypto, gVisor/Firecracker, web UI, marketplace, long-term memory,
multi-tenant scale.

---

## 3. Two tracks — pick the spirit before the sequence

### Track 1 (recommended first): FASTEST USABLE SLICE — a spike, not a framework
Per your own rule (validate with 3–5 real users before building frameworks) and MVP-first:
build the **ugliest real thing** that a design partner could run on one workflow, then let their
feedback order Track 2.

- Scope: `process.exec` (S1) + fs breadth (S2) + a Claude planner loop (P1, minimal) +
  a hardcoded policy (Pol1 as a struct, not a file) + `git`/PR (S4) — one workflow:
  *"agent edits repo, runs the test suite via exec, opens a PR; blocked if it tries to leak."*
- Skip: http (S3) unless the partner needs it, the policy *file* format, the SDK, packaging polish.
- Goal: something in front of **1–3 design partners in weeks**, not a clean architecture.
- Output: a prioritized "what actually mattered" list that reorders everything below.

### Track 2: the "proper" roadmap (only after a partner pulls it)
Candidate milestone sequence — each keeps I0/I1/I2 intact (every sink goes through the executor):

- **v1.7 — Effect breadth I:** `process.exec` (S1, confined child + tainted output) + fs breadth (S2). The hardest, highest-value primitive.
- **v1.8 — Effect breadth II:** `http.request` (S3, authorized egress) + `git`/`github.pr` (S4, the irreversible effect → confirmation/I2).
- **v1.9 — Real planner loop:** LLM-driven multi-step planner (P1) on the v1.4 sidecar seam; retries/error handling.
- **v1.10 — Policy + API:** minimal declarative policy file (Pol1) + thin SDK/CLI + audit-DAG viewer (U1).
- **v1.11 — Design-partner hardening:** one real end-to-end workflow with 1–3 partners; packaging (D1); robustness.

**Honest magnitude:** ~5 milestones of Track-2 work to a genuinely usable design-partner product —
i.e. *substantially more than what's shipped so far*, not a phase or two. Track 1 compresses the
*time-to-first-real-user* to weeks by trading polish for signal.

---

## 4. Risks / unknowns to pressure-test before committing
- **`process.exec` under Landlock+seccomp is the riskiest primitive** — confining the child, capturing+tainting output, and not re-opening an escape is genuinely hard. It may deserve its own design-gate + adversarial review like the security phases did.
- **Planner integration ≠ product.** A demo-grade planner (~70%) is not ship-grade; per your AI-build rules this needs an eval set (domain-expert ground truth, gradable rubrics) BEFORE building the loop, with a plain-prompt baseline first.
- **Is the buyer the same as the demo audience?** The security demo wows engineers; the purchaser may be a security/platform lead with different criteria. Anchor choice (§1) should be validated against *who signs*.
- **Linux-only + kernel ≥5.13** narrows the install base; acceptable for design partners, a real constraint at GTM.
