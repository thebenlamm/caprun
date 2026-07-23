# Phase 35: DESIGN Gate + Fresh Adversarial Code-Trace - Context

**Gathered:** 2026-07-18
**Status:** Ready for planning
**Mode:** Orchestrator-authored (Ben asleep; autonomous milestone run). All grey-area design forks decided here on the research's strong recommendations, with rationale, and flagged for Ben's morning review. These decisions are AUTHORITATIVE inputs to the DESIGN doc — the executor transcribes and elaborates them, it does not re-open them.

<domain>
## Phase Boundary

Produce `planning-docs/DESIGN-git-github-http-sinks.md` — the reviewed design contract for all four v1.8 sinks (`git.commit`, `git.push`, `github.pr`, `http.request`). This phase writes NO `crates/{executor,brokerd,sandbox,runtime-core}` TCB code — it is the hard gate that unblocks Phases 36-40. The doc must pin the mechanism + fail-closed default for each sink, close all 11 design-gate-blocking pitfalls with a named mechanism each, preserve I0/I1/I2, and introduce no raw `EffectRequest` path. Then a fresh **non-self, orchestrator-owned** adversarial code-trace must clear it (DESIGN-16) before any Phase 36+ code.

Grounding: `.planning/research/SUMMARY.md` + `STACK.md` + `FEATURES.md` + `ARCHITECTURE.md` + `PITFALLS.md` (all HIGH-confidence, code-grounded). This CONTEXT captures the decisions those documents recommended.
</domain>

<decisions>
## Implementation Decisions (AUTHORITATIVE — decided by orchestrator on research recommendation)

### Dispatch patterns (locked by research — both are shipped exemplars)
- **Pattern A — in-broker / broker-helper network egress** (exemplar `crates/brokerd/src/sinks/email_smtp.rs`): owns `http.request` and `github.pr`. Secret/token read from broker-local env ONLY — never a ValueNode, plan-node arg, audit-DAG literal, the confined worker, or the planner sidecar.
- **Pattern B — broker-spawned confined child** (exemplar `cli/caprun-exec-launcher` + `crates/brokerd/src/sinks/process_exec.rs`): owns `git.commit`, and `git.push` via a net-allowed variant (see Fork 1). The confined worker never `execve`s git; the broker spawns the launcher, which self-confines (Landlock + seccomp + rlimits + timeout) BEFORE `execve`ing git, captures stdout/stderr, and the broker taint-mints the output.
- **No third pattern. No raw `EffectRequest`.**

### Stack (locked by research)
- git ops → shell out to the system `git` binary via the exec-launcher (zero new TCB deps). NOT `git2` (links C libgit2) and NOT `gix` (push unimplemented).
- `github.pr` + `http.request` → raw `reqwest =0.13.4` (rustls). NOT `octocrab`.
- CA roots → compiled-in **`webpki-roots 1.0.8`** (NOT `rustls-platform-verifier`) so `env_clear()` is hermetic — no `SSL_CERT_*` / readable system cert store required.

### Effect classes (locked by roadmap + research)
- `git.commit` = **MutateReversible** — deliberate, explicitly-justified exception to the fail-closed `unknown → CommitIrreversible` default (a local commit is reversible; only push/PR are external). Survives an I1-demoted (draft-only) session.
- `git.push` = **CommitIrreversible**.
- `github.pr` = **CommitIrreversible** + explicit credential auth-grant (Fork 3).
- `http.request` GET = **Observe** (allowed even in a demoted session), BUT its response demotes the session (see mint_from_http).

### mint_from_http — the one genuinely NEW mechanism
- A new mint site `mint_from_http` taints the inbound HTTP response body as untrusted-on-arrival, rooted on a genuine `http_response_received` audit event (analogous to v1.7 `mint_from_exec` rooted on `process_exited`). It **MUST demote the session to draft-only (I1)** — decided YES per research.
- A new `TaintLabel::HttpRaw` variant (compile-forced into the exhaustive `is_untrusted()` match). git output reuses the existing exec-output taint label (git IS an exec under Pattern B).
- Anti-staple discipline: taint is genuinely propagated through the DAG (a fetched value routed into a sensitive sink arg Blocks on a real edge), never stapled at the sink. An anti-staple test is required (the §9 genuineness standard).

### The three grey-area FORKS — decided (flagged for Ben)

**FORK 1 — git.push network path: DECIDED = net-allowed confined child (Pattern B extended).**
Rationale: keeps the push credential + network egress inside a short-lived, kernel-confined child rather than the long-lived broker reference monitor — honoring "keep the broker small; broker bugs = full compromise" (CLAUDE.md residual-risks + DEC-layer-roles). The launcher gets a MINIMAL seccomp relaxation permitting only the socket syscalls needed to reach the ONE pinned remote host:port (no arbitrary egress), Landlock confined to the workspace repo, credential injected via a short-lived `GIT_ASKPASS`/env visible only to that child and `env_clear()`'d otherwise. The net-relaxation is the riskiest surface → explicitly the top item for the adversarial review to pressure-test. Alternative (in-broker `git push`) rejected: puts an unconfined git subprocess as a child of the broker and widens the reference monitor. FINALIZE + adversarially review in the doc.

**FORK 2 — rustls crypto provider: DECIDED = lean `ring` (pure-Rust) for new egress, accept `aws-lc-rs` if provider-consistency is materially cleaner.**
Rationale: "minimize untrusted C in the TCB" (project value; TCB is Rust). The net egress runs inside the TCB boundary (broker/confined child), so new C crypto is a conscious add. The planner sidecar already ships aws-lc-rs but is a SEPARATE process, so no in-process provider conflict forces a match. Low-stakes either way (both well-audited) — the DESIGN doc author picks and justifies; the review sanity-checks. Do NOT over-constrain.

**FORK 3 — github.pr human auth-grant model: DECIDED = session-scoped capability grant, separate from per-effect confirm.**
Rationale: a bearer token's authority far exceeds one PR (push/merge/cross-repo read), it opens a default-deny-net hole, and it is a broker-held secret — so confirming a PR body ≠ authorizing the credential. A distinct human action (`caprun grant <session> ...`) authorizes the broker to USE the token for that session, recorded as its own audit event, INDEPENDENT of the per-PR I2 confirm of the (sink,args). Two independent gates (capability grant + per-effect I2). Mirrors the v1.4 `ConnectionRole` capability precedent. FINALIZE the exact verb/lifetime in the doc.

### Pitfall closures the doc MUST name a mechanism for (all 11 design-gate-blocking)
1. **Tainted PR-body/commit-message exfil (marquee):** title/body/message are I2-sensitive sink args (reuse CONTENT-01 content-sensitivity); taint genuinely propagated; verbatim provenance shown at confirm.
2. **git config/hook RCE:** hardcoded neutralization in the launcher — `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`, `-c core.hooksPath=/dev/null`, no aliases, `GIT_TERMINAL_PROMPT=0`, `env_clear()`'d child.
3. **Swapped-remote push:** push remote URL + branch captured from TRUSTED intent at session creation and passed explicitly to the child — NEVER resolved from the untrusted repo's `.git/config`.
4. **`--force` / destructive refspec:** hard-denied (no `--force`/`--force-with-lease`, no `:refspec` deletion, no `+` force-refspec); remote + refspec are I2-gated sink args.
5. **Credential leak/flow:** token/push-cred lives in broker-local env only; never a ValueNode/plan-arg/audit-literal/worker/planner-env; short-lived injection to the child only.
6. **Token over-scoping:** doc specifies MINIMAL scopes (fine-grained PAT: Pull requests:write + Contents:read) and states scope is an operator responsibility surfaced at grant time.
7. **http SSRF:** resolve-and-pin the destination IP; deny loopback/RFC1918/link-local/CGNAT/cloud-metadata (169.254.169.254)/ULA/IPv6-mapped equivalents; NO redirect following by default; reject `userinfo@`, non-`https` schemes, and IP-encoding tricks; host allowlist; connect to pinned IP with SNI/Host = original host.
8. **http response not minted / stapled taint:** `mint_from_http` at arrival rooted on a real event + session demotion; anti-staple test proving the downstream Block.
9. **net-deny widening:** the confined WORKER never gains net; egress is broker/confined-net-child only; the net relaxation is scoped to a single pinned host:port for git.push and to the broker for http/github.
10. **push/PR effect-class:** pinned per above; git.commit's MutateReversible exception explicitly justified.
11. **Replay / duplicate-PR CAS:** content-derived idempotency key (owner/repo/base/head/title/body digest) committed to a CAS table BEFORE the GitHub API call — mirroring v1.6 HARDEN-03's Allowed-path replay defense.

### Recurring audit-gap discipline (P33/P34 — MANDATORY for the new confirm-releasable sinks)
git.push and github.pr are CommitIrreversible + confirm-releasable. The doc MUST require, for each: the confirm-release path writes the TERMINAL AUDIT EVENT **before** the terminal state (never a terminal STATE before the EVENT that justifies it), with a `prepare_*` precheck. This is the RECURRING MAJOR class that a passing verifier + green gates missed twice (v1.7 P33 file.write, P34 process.exec) and only the fresh adversarial trace caught.

### env_clear TLS policy (closes the folded-in v1.7 todo, ENV-01 — realized in Phase 40)
webpki-roots compiled-in ⇒ `env_clear()` is hermetic; the surviving allowlist for any TLS-egress process is only `HTTPS_PROXY`/`NO_PROXY` (behind a proxy) + minimal `PATH`/locale. MUST be validated by a LIVE HTTPS run (the only place the TLS-env regression manifests) — offline/mocked tests do not catch it.
</decisions>

<code_context>
## Existing Code Insights

- `crates/brokerd/src/sinks/email_smtp.rs` — Pattern A exemplar (in-broker network egress, broker-only secret sourcing "D-04").
- `crates/brokerd/src/sinks/process_exec.rs` + `cli/caprun-exec-launcher` — Pattern B exemplar (broker-spawned confined child, self-confine-before-execve, capture output).
- `crates/brokerd/src/quarantine.rs` (or equivalent) — `mint_from_read`/`mint_from_intent`/`mint_from_derivation`/`mint_from_exec` live here; `mint_from_http` joins them.
- `crates/executor/src/sink_sensitivity.rs` — hardcoded `expected_role()` + sensitivity table; new sink arg rows added here (I2 stays hardcoded).
- `crates/executor/src/sink_schema.rs` (`KNOWN_SINKS`) — new sink registration rows.
- `crates/runtime-core/src/` — `TaintLabel` (add `HttpRaw`, compile-forced exhaustive match), `Effect`/effect-class (`effect.rs` already has a `GitPush` — verify), `PlanNode`/`ValueNode`.
- `cli/caprun/src/main.rs:~322` — the planner-sidecar spawn NOT env_clear'd (the ENV-01 gap; realized Phase 40).
- `crates/brokerd/src/confirmation.rs` — confirm-release path; the P33/P34 terminal-event-before-state discipline lives here.
- Verification tooling: `scripts/mailpit-verify.sh` (extend for a mock GitHub/HTTP endpoint in Phase 40).
</code_context>

<specifics>
## Specific Ideas

The DESIGN doc should be structured for the adversarial reviewer: a per-sink section (dispatch pattern, effect-class, sink args + which are I2-sensitive, taint flow, confinement), a threat-model section mapping each of the 11 pitfalls → named mechanism, the three fork decisions with rationale, the new `TaintLabel`/mint_from_http/CAS/auth-grant mechanisms, and an explicit "does not weaken I0/I1/I2, introduces no raw EffectRequest" invariant-preservation section. Cite the exemplar source files so the reviewer can trace claims to code.

Out of scope for the doc (v1.9+): http write egress (POST), PR merge/comment, real LLM planner loop, declarative policy file, git2/gix, octocrab.
</specifics>

<deferred>
## Deferred Ideas
None — decisions front-loaded above for the autonomous run.
</deferred>
