# Requirements: caprun v1.8 — Git/GitHub Adapters (Effect Breadth II)

**Defined:** 2026-07-18
**Core Value:** A kernel-confined worker can only cause external effects through broker-mediated plan nodes, and a genuinely-propagated taint chain (raw/inbound read → ValueNode → sensitive sink arg) deterministically blocks value-injection at the sink (I2).
**Anchor use case:** Anchor A — the Safe Coding Agent (agent edits a repo → runs tests via `process.exec` → commits → opens a GitHub PR → optional allowlisted HTTP fetch; every irreversible/external effect I2-gated, value-injection blocked).

## v1 Requirements

Requirements for v1.8. Each maps to exactly one roadmap phase. REQ-IDs continue
numbering from prior milestones (DESIGN and LIVE families continue; GIT/GITHUB/HTTP/ENV
are new families starting at 01).

### Design Gate

- [ ] **DESIGN-15**: A DESIGN doc (`planning-docs/DESIGN-git-github-http-sinks.md`) pins the mechanism + fail-closed default for all four new sinks — effect-class per sink; `mint_from_http` inbound-taint + session demotion; git config/hook neutralization surface; git.push destination pinning + credential-injection mechanism; the SSRF resolve-and-pin model; the github.pr human auth-grant model; the `env_clear()` TLS-cert allowlist policy; duplicate-PR CAS semantics; and the new `TaintLabel` variants — explicitly closing all 11 design-gate-blocking pitfalls.
- [ ] **DESIGN-16**: The DESIGN doc clears a fresh **non-self** adversarial code-trace (orchestrator-owned, not a gsd-executor) before any `crates/{executor,brokerd,sandbox,runtime-core}` TCB code is written — per the unbroken v1.0–v1.7 design-gate-first precedent.

### Git Sinks

- [ ] **GIT-01**: A `git.commit` sink commits staged workspace changes via the broker-spawned confined-child launcher (reusing the v1.7 `caprun-exec-launcher` + `mint_from_exec` pattern), classified **MutateReversible** (survives an I1-demoted session); the commit message's taint genuinely propagates downstream (not re-minted clean); git system config and hooks are neutralized in the child (`GIT_CONFIG_NOSYSTEM`, `core.hooksPath=/dev/null`, no aliases, `env_clear()`'d).
- [ ] **GIT-02**: A `git.push` sink pushes to a remote, classified **CommitIrreversible**; the remote + branch are pinned to the session's trusted intent-origin and passed explicitly (never resolved from the untrusted repo's `.git/config`); `--force` and ref-deletion are hard-denied; remote + refspec are I2-gated sink args.
- [ ] **GIT-03**: A tainted `git.push` remote/refspec is deterministically Blocked at the sink (I2) and releasable only by single-shot human confirmation; the confirm-release path writes the terminal audit event **before** the terminal state (the recurring P33/P34 audit-gap discipline).

### GitHub

- [ ] **GITHUB-01**: A `github.pr` sink creates a GitHub pull request via a broker-held session bearer token (the token is read from broker-local env only — never present in the confined worker, the planner sidecar, a ValueNode, or the audit-DAG literal), classified **CommitIrreversible**.
- [ ] **GITHUB-02**: `github.pr` requires an explicit human auth-grant for the credential — a step beyond single-shot confirm (a token's authority exceeds one PR); confirming a PR body does not by itself authorize the token's use.
- [ ] **GITHUB-03**: A tainted PR title/body section is deterministically Blocked (I2, reusing CONTENT-01 content-sensitivity) — the marquee secret-exfil-via-PR-text attack; the verbatim, provenance-annotated title/body is shown to the human at confirm.
- [ ] **GITHUB-04**: A replayed `github.pr` submission creates at most one PR (content-derived idempotency CAS committed before the API call — mirroring the v1.6 HARDEN-03 Allowed-path replay defense).

### HTTP Egress

- [ ] **HTTP-01**: An `http.request` sink performs a broker-mediated outbound **GET to an allowlisted host only** (read-only; POST/write egress explicitly out of scope for v1.8), classified **Observe**; the request `url` is an I2-gated sink arg.
- [ ] **HTTP-02**: The HTTP response body is minted untrusted-on-arrival via a new `mint_from_http` mint site (rooted on a genuine `http_response_received` audit event) and demotes the session to draft-only (I1); a fetched value later routed into a sensitive sink arg is Blocked on a **genuinely-propagated, non-stapled** taint chain (the §9 genuineness standard, with an anti-staple test).
- [ ] **HTTP-03**: `http.request` defends against SSRF — resolve-and-pin the destination IP, deny loopback/RFC1918/link-local/cloud-metadata ranges, do not follow redirects by default, and reject `userinfo@`/IP-encoding allowlist-bypass tricks.

### Egress Env Hygiene

- [ ] **ENV-01**: The `caprun-planner` sidecar spawn is `env_clear()`'d and given only the minimal env it needs; all new broker-side TLS egress uses compiled-in `webpki-roots` so `env_clear()` is hermetic (no `SSL_CERT_*` / readable system store required), validated by a **live** HTTPS run (the only place the TLS-env regression is caught). Closes the deferred v1.7 `2026-07-18-planner-sidecar-env-clear` todo.

### Live Proof

- [ ] **LIVE-03**: A composed agent workflow proven on real Linux — `process.exec` (test) → filesystem edit → `git.commit` → `git.push` → `github.pr`, plus an `http.request` GET leg — with every step gated, tainted, and audit-DAG-chained, and `verify_chain` true across the run.
- [ ] **LIVE-04**: The composed run carries adversarial attack legs, each deterministically Blocked with `verify_chain` true — (a) tainted push remote/refspec, (b) tainted PR-body section, (c) tainted GET url (SSRF/exfil) — plus a post-`env_clear()` **live** HTTPS call that succeeds; the full-workspace regression is green on real Linux with no regression to v1.0–v1.7.

## Future Requirements

Deferred to v1.9+. Tracked but not in this roadmap.

### HTTP / GitHub breadth
- **HTTP-W-01**: `http.request` write egress (POST/PUT) with request-body I2 gating.
- **GITHUB-W-01**: `github.pr` merge/comment; multi-repo/fork PRs; OAuth/GitHub-App token provisioning.

### Planner / Product
- **PLANNER-LOOP-01**: A real LLM planner loop (multi-step tool-use on the v1.4 sidecar seam) driving the composed workflow.
- **POLICY-01**: A declarative policy file for per-session sink allowlists + arg constraints (hosts, paths, repos).
- **SDK-01**: A thin SDK/CLI + audit-DAG viewer.

## Out of Scope

Explicitly excluded from v1.8. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| `http.request` write egress (POST) | Read-only GET first; write egress + request-body I2 deferred to v1.9 (PROJECT scope). |
| `git2`/libgit2 or `gix` for git ops | Shelling to the `git` binary via the confined-child launcher adds zero new TCB deps; `git2` links C, `gix` push is unimplemented. |
| `octocrab` GitHub SDK | Reuse the already-vetted `reqwest`; one REST POST doesn't justify a new SDK dep tree in the TCB. |
| Real LLM planner loop driving the composed workflow | A deterministic/stub planner suffices to prove the sinks; planner loop is a v1.9 concern. |
| Cedar policy engine / declarative policy file | I2 stays hardcoded in Rust; per-session allowlists deferred to v1.9. |
| GitHub PR merge/comment, multi-repo/fork PRs | Beyond the anchor demo (create one PR); deferred. |
| Cross-host delegation, gVisor/Firecracker, web UI, marketplace | Standing out-of-scope (v2/v3 concerns). |

## Traceability

Which phases cover which requirements. Populated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| DESIGN-15 | Phase 35 | Pending |
| DESIGN-16 | Phase 35 | Pending |
| GIT-01 | Phase 36 | Pending |
| GIT-02 | Phase 39 | Pending |
| GIT-03 | Phase 39 | Pending |
| GITHUB-01 | Phase 38 | Pending |
| GITHUB-02 | Phase 38 | Pending |
| GITHUB-03 | Phase 38 | Pending |
| GITHUB-04 | Phase 38 | Pending |
| HTTP-01 | Phase 37 | Pending |
| HTTP-02 | Phase 37 | Pending |
| HTTP-03 | Phase 37 | Pending |
| ENV-01 | Phase 40 | Pending |
| LIVE-03 | Phase 40 | Pending |
| LIVE-04 | Phase 40 | Pending |

**Coverage:**
- v1 requirements: 15 total
- Mapped to phases: 15
- Unmapped: 0

---
*Requirements defined: 2026-07-18*
*Last updated: 2026-07-18 after roadmap creation (Phases 35-40, 15/15 mapped)*
