# Phase 38: `github.pr` Sink - Context

**Gathered:** 2026-07-18
**Status:** Ready for planning
**Mode:** Orchestrator-authored (autonomous v1.8). Design fixed by the CLEARED gate doc
`planning-docs/DESIGN-git-github-http-sinks.md` §4 (+ §3.6/§5 reused egress, §6 pitfalls
1/5/6/9/11, §7, §8, §9 confirm-release, §10). Reuses the Phase-37 http egress + mint infra.
Do not re-open design decisions.

<domain>
## Phase Boundary
Implement the `github.pr` sink: broker-mediated GitHub PR creation (one REST POST) via a
broker-held session bearer token, gated by a NEW human auth-grant (distinct from per-effect
confirm), with tainted title/body Blocked (CONTENT-01) and a duplicate-PR CAS. Reuses Phase-37
Pattern-A egress (reqwest rustls/ring) + the SSRF resolve-and-pin. Requirements: GITHUB-01..04.
Authoritative design: DESIGN §4 (+ §4.1 base-URL pin, §4.3 auth-grant FORK-3, §4.5 CAS, §9 confirm-release).
</domain>

<decisions>
## Implementation Decisions (FIXED by the cleared DESIGN gate — §4, §8, §9)

1. **Dispatch = Pattern A, one REST `POST /repos/{owner}/{repo}/pulls` via reqwest** (reuse
   Phase-37 http egress client). Effect-class = `CommitIrreversible`. Base URL = a FIXED
   broker-owned trusted-config constant (`https://api.github.com`), sourced like the SMTP
   endpoint (D-04) — NEVER from a resolved/tainted arg or from owner/repo; rides the §3.6
   resolve-and-pin + host allowlist (base-URL SSRF pin — the round-2 gate fix, MAJOR-4).
2. **Credential hygiene (P5/P8):** bearer token read from broker-local env ONLY — never a
   ValueNode, plan-node arg, audit-DAG literal, the confined worker, or the planner sidecar.
   Audit events carry OPAQUE payloads (effect_id + static marker; token + raw API response
   text never enter the hash chain) — mirror email_smtp's opaque `_succeeded`/`_failed`.
3. **FORK-3 auth-grant (NEW mechanism, GITHUB-02):** a session-scoped capability grant,
   SEPARATE from per-effect confirm. A distinct human action `caprun grant <session> ...`
   authorizes the broker to USE the token for that session, recorded as its OWN audit event,
   session-scoped (lifetime = the Session; does not persist across sessions). A PR CANNOT be
   created on a bare confirm alone — absent a live grant the sink Denies, independent of any
   per-PR confirm. TWO independent gates: capability grant AND per-effect I2 confirm. Mirrors
   the v1.4 ConnectionRole capability precedent.
4. **Tainted-PR-body block (marquee P6, GITHUB-03):** `title`/`body` are content-sensitive
   sink args (reuse CONTENT-01 `is_content_sensitive`) — a value assembled from untrusted
   content (http_response/ExecRaw/doc_fragment-tainted) Blocks under the UNMODIFIED
   collect-then-Block loop. `owner`/`repo`/`base`/`head` are routing-sensitive (I2-gated).
   Verbatim, provenance-annotated title/body shown to the human at confirm.
5. **Token scoping (P6-overscope):** MINIMAL — a fine-grained PAT with `Pull requests: write`
   + `Contents: read` only — stated as operator responsibility surfaced at grant time.
6. **Duplicate-PR CAS (P16/GITHUB-04):** a content-derived idempotency key = digest over
   `(owner, repo, base, head, title, body)`, committed to a CAS table BEFORE the API call
   (mirror v1.6 HARDEN-03 / email_smtp's CAS): CAS + attempt-append commit atomically before
   any socket opens; a PRIMARY-KEY violation on replay IS the CAS → at most one PR. The key
   MUST be DERIVED from resolved plan-node content, NEVER a new PlanNode field, NEVER keyed on
   effect_id. Accepted residual (D-08): at-most-once PER PLAN NODE; and the crash-window
   lost-effect residual (§11) — do NOT add a clear-key-on-failure path.
7. **Confirm-release (P33/P34, §9):** github.pr is CommitIrreversible + confirm-releasable.
   A `prepare_github_pr` precheck runs BEFORE confirm() appends `confirm_granted` (Step 5) and
   burns the one-shot (Step 6), folding every fallible pre-effect leg through the single
   terminal-event branch — terminal EVENT before terminal STATE (the recurring MAJOR class).
   EXTEND the Step-4.75 entry-guard allow-list (`confirmation.rs:836-845`) to admit github.pr
   (a confirm-releasable sink NOT on the list is denied at the guard). Regression test: no
   dangling `confirm_granted`-without-terminal-event.
</decisions>

<code_context>
## Existing Code Insights
- `crates/brokerd/src/sinks/http_request.rs` (Phase 37) — reuse the pinned reqwest client + SSRF
  resolve-and-pin for the POST to api.github.com.
- `crates/brokerd/src/sinks/email_smtp.rs` — D-04 broker-only secret sourcing + opaque audit + CAS caller model.
- `crates/brokerd/src/confirmation.rs` — the confirm() Step 4.75 entry guard (~836-845, extend for github.pr),
  Step 4.8 prepare_process_exec pattern (~847-866, mirror as prepare_github_pr), Step 5/6/7,
  the malformed-args-does-not-burn regression test (~1965).
- `crates/executor/src/{sink_schema,sink_sensitivity}.rs` — github.pr KNOWN_SINKS row (owner/repo/base/head/title/body),
  CommitIrreversible class, title/body content-sensitive, owner/repo/base/head routing-sensitive.
- `crates/brokerd/src/audit.rs` / server.rs — the duplicate-PR CAS table (migration) + the auth-grant event;
  reuse HARDEN-03's CAS pattern.
- `cli/caprun` — the `caprun grant` CLI verb (session auth-grant); mirror existing verbs (confirm/deny).
- Tests (maximize host-portable): tainted-title/body Blocks; no-grant→Deny (bare confirm insufficient);
  duplicate-PR CAS at-most-once; confirm-release terminal-event-before-state (no dangling confirm_granted);
  token/response never in audit literal. Live GitHub POST → a MOCK endpoint in Phase 40 (no real GitHub creds).
</code_context>

<specifics>
## Specific Ideas
- The github.pr network POST is broker-side; the token custody + auth-grant are the load-bearing new bits.
- The auth-grant should be a durable session-scoped capability the broker checks at github.pr dispatch
  (Deny without it) — a NEW audit event type + a session capability record. Distinct from the confirm one-shot.
- Reuse Phase-37 SSRF/base-URL pin for the POST (MAJOR-4 fix: base host fixed + resolve-and-pin).
- No raw EffectRequest; sink sensitivity hardcoded; check-invariants green.
- Phase 40's composed live-proof will POST to a mock GitHub endpoint (no real GitHub creds/repo needed) —
  keep the endpoint base overridable for the test harness the way SMTP host is (CAPRUN_* env for the mock).
</specifics>

<deferred>
## Deferred Ideas
- PR merge/comment, multi-repo/fork PRs, OAuth/GitHub-App token provisioning → v1.9+.
- Real live GitHub POST behavioral proof → Phase 40 (mock endpoint).
</deferred>
