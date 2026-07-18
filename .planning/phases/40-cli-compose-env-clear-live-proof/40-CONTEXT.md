# Phase 40: CLI Compose, Sidecar env_clear() & Composed Live Proof (v1.8 DONE) - Context

**Gathered:** 2026-07-18
**Status:** Ready for planning
**Mode:** Orchestrator-authored (autonomous v1.8). This is the v1.8 DONE gate. Rescoped: git.push
(Phase 39) is DEFERRED to v1.9 (`planning-docs/DECISION-git-push-deferral-v1.8.md`), so the composed
workflow is exec→fs→git.commit→github.pr(mock)+http GET — the real push step is out of scope.

<domain>
## Phase Boundary
Prove the Safe Coding Agent workflow end-to-end on REAL LINUX with the 3 shipped sinks, close the
ENV-01 sidecar env_clear, and prove the adversarial attack legs are deterministically Blocked.
Requirements: ENV-01, LIVE-03, LIVE-04.
</domain>

<decisions>
## Implementation Decisions

1. **ENV-01 — env_clear() the caprun-planner sidecar spawn** (`cli/caprun/src/main.rs:~314-335`).
   Today the sidecar is NOT env_clear'd (unlike the worker). env_clear it + set ONLY the minimal env
   it needs: OPENAI_API_KEY (explicit), PLANNER_SOCK, CAPRUN_PLANNER_MODEL, a minimal PATH, and
   HTTPS_PROXY/NO_PROXY IF present (proxy passthrough). This is now SAFE because Phase 37's MAJOR-1
   fix switched the sidecar's reqwest to rustls-no-provider + ring + compiled-in webpki-roots — so
   TLS cert validation needs NO SSL_CERT_* / system store → env_clear is HERMETIC for certs. Drop the
   dead `SESSION_ID` worker-spawn allowlist entry (worker.rs never reads it — HARD-03 removed the field).
2. **The "live HTTPS run" that validates hermetic env_clear (ENV-01):** a broker-side `http.request`
   GET to a real allowlisted HTTPS host (Phase 37 egress, same webpki-roots config) run AFTER env_clear
   — this exercises the exact TLS-cert path and proves env_clear + webpki-roots is hermetic (no cert
   env needed). The full live-OpenAI SIDECAR run (CAPRUN_PLANNER=llm) additionally validates the
   sidecar's own env_clear IF an OPENAI_API_KEY is available in the run env; if no key, the sidecar
   env_clear is verified structurally (spawn + allowlist correct) + the broker-side live HTTPS GET
   stands as the webpki-roots-hermetic proof, and the live-OpenAI confirmation is flagged for a
   key-present run (do NOT claim the LLM-sidecar live path passed if it was skipped for lack of a key).
3. **LIVE-03 composed workflow (real Linux, mock GitHub):** a single composed run — process.exec (run
   a test/command, output tainted) → filesystem edit (file.write) → git.commit (real confined commit)
   → github.pr (POST to a MOCK GitHub endpoint via CAPRUN_GITHUB_API_BASE override — still rides
   validate_url/allowlist/resolve-and-pin; the mock accepts the PR create, standing in for the
   pushed-branch precondition) + an http.request GET leg — every step gated, tainted, audit-DAG-chained,
   verify_chain true across the run.
4. **LIVE-04 adversarial legs (each deterministically Blocked, verify_chain true):**
   (a) a tainted PR title/body section (fetched-via-http or exec-derived untrusted value routed into
       github.pr title/body) → Blocked (I2/CONTENT-01);
   (b) a tainted GET url (SSRF range / non-allowlisted host / secret-in-query) → Deny/Block;
   (c) a tainted commit message (untrusted-derived) routed into git.commit message → Blocked.
   Plus: the post-env_clear live HTTPS call succeeds; the full-workspace regression is green on real
   Linux with NO regression to v1.0–v1.7.
5. **Harness = extend `scripts/mailpit-verify.sh`** (or a sibling) to also stand up a MOCK GitHub HTTP
   endpoint (a tiny local HTTP server the github.pr POST hits via CAPRUN_GITHUB_API_BASE) alongside the
   Mailpit SMTP sidecar. Must run the standard unprivileged rust:1 container recipe. CRITICAL: run
   `cargo build --workspace` BEFORE `cargo test` (cargo-test-workspace-missing-sibling-binary: the
   confined-child launcher + worker sibling binaries must be placed, else git.commit/exec spawn fails).
6. **The orchestrator runs the CLOSING gate directly** (not delegated): re-run the composed live proof
   capturing the TRUE exit code BEFORE any pipe, asserting on named tests + counts (verification-exit-
   code-through-pipe lesson), per the v1.3/v1.5 coordinator-gate precedent.
</decisions>

<code_context>
## Existing Code Insights
- `cli/caprun/src/main.rs` — the sidecar spawn (env_clear + allowlist) + worker spawn (already env_clear'd).
- `crates/brokerd/src/sinks/{http_request,github_pr,git_commit}.rs` — the 3 shipped sinks; github.pr base
  overridable via CAPRUN_GITHUB_API_BASE (mock); http.request allowlist.
- `scripts/mailpit-verify.sh` — the Linux verification harness to extend (Mailpit sidecar + rust:1 container
  on a user-defined docker network; installs libssl-dev/pkg-config).
- Prior composed live-proof precedents: v1.3 ACCEPT-01, v1.4 GATE, v1.7 LIVE-01/02 (composed multi-leg,
  one shared audit.db, per-session verify_chain).
- A composed acceptance integration test (Linux-gated) that drives caprun end-to-end through the 3 sinks +
  the mock endpoints, asserting the gated/tainted/Blocked outcomes + verify_chain.
</code_context>

<specifics>
## Specific Ideas
- The mock GitHub endpoint can be a tiny axum/hyper server (or reuse an existing test-fixture pattern) that
  responds 201 with a fake PR JSON to POST /repos/*/pulls; it exists only so the github.pr POST completes
  and the opaque success event + CAS are exercised live. No real GitHub creds.
- Honesty (DOC-style): the milestone record must state plainly that v1.8 proves edit→commit→open-PR (mock)
  + authorized HTTP fetch, that git.push is deferred (real push NOT proven), and whether the live-OpenAI
  sidecar leg ran (key present) or was structurally-verified-only (no key).
- Keep the true-exit-before-pipe discipline; assert on named tests + counts, never on `exit 0` through a pipe.
</specifics>

<deferred>
## Deferred Ideas
- git.push + its live/adversarial-push legs → v1.9.
- Real live GitHub (non-mock) PR creation → optional post-milestone config-swap (like live SES was for v1.3).
</deferred>
