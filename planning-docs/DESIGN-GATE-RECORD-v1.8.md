# DESIGN Gate Record — v1.8 (Git/GitHub Adapters, Effect Breadth II)

**Phase:** 35 (DESIGN Gate + Fresh Adversarial Code-Trace)
**Requirement:** DESIGN-16
**Doc under review:** `planning-docs/DESIGN-git-github-http-sinks.md`
**Gate discipline:** No `crates/{executor,brokerd,sandbox,runtime-core}` TCB code for
any v1.8 sink may be written until this doc clears a fresh, **non-self**,
**orchestrator-owned** adversarial code-trace with every finding resolved — the unbroken
precedent (v1.0 P2, v1.2 P8, v1.3 P12, v1.4 P18, v1.5 P23, v1.6 P26, v1.7 P31).

**Reviewer independence:** The DESIGN doc was authored by a `gsd-executor` (Plan 35-01)
from the orchestrator's authoritative `35-CONTEXT.md` decisions. The adversarial
code-trace was run by a **fresh Fable-5 agent** (a different model, non-self), spawned and
owned by the **orchestrator** (not a gsd-executor) — satisfying DESIGN-16's
"orchestrator-owned, not a gsd-executor" requirement and the standing
`fresh-context-adversarial-review` discipline that has caught 8 real defects across prior
milestones. This is the 9th.

**Method:** the reviewer traced every load-bearing `file:line` citation in the doc against
the real code, and adversarially attacked the design (git.push net-egress feasibility,
credential leak via captured output, mint_from_http ordering/demotion, P33/P34
confirm-release, github.pr POST SSRF, http=Observe exfil, git.commit=MutateReversible
laundering, duplicate-PR CAS, raw-EffectRequest / I2-bypass). Not a prose-read.

---

## Round 1 — VERDICT: CHANGES REQUIRED (1 BLOCKER, 3 MAJOR, 1 MINOR, 1 NIT)

**Citation audit:** all spot-checked `file:line` citations accurate — the doc's
groundedness held up under a real code trace.

| # | Sev | Finding | Resolution in the revised doc |
|---|-----|---------|-------------------------------|
| 1 | **BLOCKER** | "seccomp relaxation permitting ONLY the socket syscalls to reach ONE pinned host:port" is **not expressible** — seccomp-bpf gates syscall numbers/scalar registers only; `connect()`'s destination is behind a pointer it cannot deref; `socket()` exposes only the family; Landlock net rules need kernel 6.7 (>5.13 floor) and filter by port only. A net-allowed child with a live push credential = arbitrary-egress exfil primitive. | **FORK-1 re-decided (§2.1):** git.push child stays FULLY net-denied (no seccomp relaxation); destination pin moved to the broker's application-layer resolve-and-pin egress (the §3.6 model the reviewer confirmed sound). §2.3/§6-row9/§11-Open-1 updated. HARD CONSTRAINT: Phase 39 must enforce the pin in a broker/netfilter layer (that can see the destination), and **defer git.push entirely rather than ship arbitrary child egress** if no unprivileged mechanism proves feasible. |
| 2 | **MAJOR** | git.push confirm surfaces only `remote`/`refspec`, never the pushed payload — breaks the "human sees what leaves the boundary" property; a worker-written file's bytes (never an I2 arg) can be committed + pushed. | **New §2.7:** git.push confirm surfaces the pushed commit range + a provenance summary flagging tainted-derived files. Documented as an accepted residual in §11 (surfaces for human judgment; does not auto-Block untainted-but-sensitive content — the human's call at confirm). |
| 3 | **MAJOR** | git.push captured child stderr can echo credential/URL/proxy-auth material and is minted as an ExecRaw taint value (no scrub), unlike github.pr's opaque rule. | **§2.5:** git.push captured output follows github.pr's opaque/scrub discipline (not process.exec's mint-the-output), + a regression test asserting no credential/URL substring survives into the value store or audit chain. |
| 4 | **MAJOR** | github.pr POST base-URL is not SSRF-pinned; §3.6 defense is GET-scoped. | **§4.1 + §8 row:** the GitHub API base is a fixed broker-owned trusted-config constant (never from a resolved/tainted arg), riding the §3.6 resolve-and-pin + host allowlist. |
| 5 | MINOR | duplicate-PR CAS crash window (CAS-commit → API-complete) loses the PR and blocks retry under the same key — the intended at-most-once tradeoff, but undocumented. | **§11:** documented as an accepted residual + a "MUST NOT add clear-key-on-failure" warning (that would reintroduce the duplicate-send hole). |
| 6 | NIT | mark `http.request` `url` content-sensitive too (query-string exfil defense-in-depth). | **§8:** `url` now routing- **and** content-sensitive. |

**Doc revision commit:** `5a113a7` (`docs(35): resolve v1.8 DESIGN adversarial-trace findings ...`). `scripts/check-invariants.sh` exits 0 against the revised doc; `git status --porcelain crates/ cli/` empty (no TCB code).

---

## Round 2 — confirmatory re-review of the revised sections

**VERDICT: CHANGES REQUIRED** — all six round-1 findings confirmed **genuinely resolved**
(traced against the same real code; the seccomp/`exec_child_filter` net-deny at
`sandbox/src/seccomp.rs:147-207`, `landlock.rs:20` ABI-V3, the capture→`mint_from_exec`
default, the D-04 endpoint sourcing, and the `confirmation.rs:824-845` entry guard all
re-verified accurate). One **new MAJOR** raised: the round-1 edit fixed §2's body but left
three summary/framing passages still asserting the withdrawn "net-allowed confined child"
model — an internal contradiction on the riskiest surface in a decision-pinning gate:
- §0 scope list (git.push "net-allowed confined child"),
- the Pattern-B definition ("net-allowed variant"),
- the §2 section title,
- the §11 accepted-residual bullet ("confined child WITH network … net-relaxation").

**Round-2 citation audit:** all new citations accurate; no stale/wrong new citations.

### Round-2 MAJOR resolution (editorial reconciliation)

All four passages reconciled to §2.1's re-decision (net-denied child + broker-mediated
resolve-and-pin egress; the residual is now "broker-mediated egress is a new trust posture
/ git.push deferred if no unprivileged destination-pin proves feasible"). A grep for
`net-allowed`/`net-relaxation`/`WITH network` confirms the ONLY remaining occurrence is the
intentional quotation of the *rejected* decision inside the §2.1 ⚠ correction block. This
was a purely editorial consistency fix implementing the reviewer's exact remediation — no
new design content — so it does not reopen the adversarial round; the round-2 review had
already confirmed every design mechanism sound and every citation accurate.

Doc fix commit: `<filled at commit>`. `scripts/check-invariants.sh` exits 0; TCB untouched
(`git status --porcelain crates/ cli/` empty).

---

## GATE CLEARED ✅

**DESIGN-16 satisfied.** `planning-docs/DESIGN-git-github-http-sinks.md` has cleared a
fresh, non-self, orchestrator-owned adversarial code-trace (2 rounds): all round-1 findings
(1 BLOCKER, 3 MAJOR, 1 MINOR, 1 NIT) resolved, the round-2 consistency MAJOR reconciled,
every load-bearing `file:line` citation verified against real code across both rounds, and
no `crates/{executor,brokerd,sandbox,runtime-core}` / `cli/` code exists yet. The
design-gate discipline is satisfied; **Phases 36-40 are unblocked.**

**Standing corrections carried into implementation (Phases 36-40 MUST honor):**
1. **git.push child stays fully net-denied; destination pin is broker-mediated
   (resolve-and-pin), never seccomp.** Defer git.push (Phase 39) entirely if no
   fully-unprivileged destination-pinning mechanism proves feasible — never ship arbitrary
   child egress. (FLAG FOR BEN — this is a scope risk on Phase 39.)
2. git.push confirm surfaces the pushed diff + tainted-file provenance (§2.7).
3. git.push captured output follows the opaque/scrub discipline (§2.5).
4. github.pr POST base-URL pinned to fixed broker trusted-config (§4.1).
5. `prepare_git_push`/`prepare_github_pr` + entry-guard extension for the P33/P34
   confirm-release audit-gap class (§9).
6. Gate 3 `mint_from_http(` extension in the same commit that introduces the mint (§10).

**Recorded:** 2026-07-18, orchestrator-owned (fresh Fable-5 reviewer, non-self).
