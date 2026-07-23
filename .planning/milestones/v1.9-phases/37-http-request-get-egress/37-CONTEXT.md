# Phase 37: `http.request` GET Egress - Context

**Gathered:** 2026-07-18
**Status:** Ready for planning
**Mode:** Orchestrator-authored (autonomous v1.8). Design fixed by the CLEARED gate doc
`planning-docs/DESIGN-git-github-http-sinks.md` §3 (+ §5 crypto/TLS, §6 pitfalls 7/8/9,
§7, §8, §10 Gate-3 mandate). This is the highest-NOVELTY v1.8 phase — the ONE genuinely new
mechanism (`mint_from_http`). Do not re-open design decisions.

<domain>
## Phase Boundary
Implement the read-only `http.request` GET sink (Pattern A, in-broker egress): an
allowlisted-host GET whose response body is minted untrusted-on-arrival via a NEW
`mint_from_http` mint site (rooted on a new `http_response_received` audit Event) that
DEMOTES the session (I1), defended against SSRF by resolve-and-pin. Establishes the
inbound-mint mechanism that Phase 38 (github.pr) reuses. Requirements: HTTP-01, HTTP-02, HTTP-03.
Authoritative design: DESIGN §3 (+ §5.1 crypto=ring, §5.2 webpki-roots TLS, §10 Gate-3).
</domain>

<decisions>
## Implementation Decisions (FIXED by the cleared DESIGN gate — §3, §5, §10)

1. **Dispatch = Pattern A (in-broker/broker-helper egress), `reqwest =0.13.4` rustls, GET only.**
   Same in-broker-egress shape as `email_smtp.rs` (the only existing net-egress sink). NOT in
   a confined child, NEVER in the worker. POST/write egress is OUT of scope (v1.9+).
2. **Effect-class = `Observe`** (first real Observe sink; only `test.observe` is Observe today).
   Allowed even in a draft session — BUT the response demotes it (see #3).
3. **`mint_from_http` — the ONE new mint site** (in `crates/brokerd/src/quarantine.rs`, mirroring
   `mint_from_read`/`mint_from_exec`). EXACT order (non-stapled genesis):
   (a) append a real `http_response_received` audit Event FIRST (new event type, via `append_event`);
   (b) THEN `ValueStore::mint` the response body with `provenance_chain=[that Event id]`
   (provenance_chain[0] EQUALS the event id — non-stapled, like mint_from_read's anchor identity),
   taint `[ExternalUntrusted, HttpRaw]`, `origin_role="http_response"`;
   (c) DEMOTE the session to draft-only (I1) — same atomic in-`conn` demotion mint_from_read does
   (UPDATE sessions SET status='Draft' + a session_demoted Event chained onto the mint event).
4. **`TaintLabel::HttpRaw`** — new enum variant in `runtime-core/src/plan_node.rs`, mirroring
   `ExecRaw`. Compile-forced into `is_untrusted()`'s exhaustive `match self` (NO wildcard) — the
   compiler forces it into the untrusted arm.
5. **Anti-staple discipline (§9 genuineness):** a fetched value routed into a sensitive sink arg
   Blocks on a REAL DAG edge rooted at `http_response_received`, never stapled. REQUIRED anti-staple
   test: assert `store.resolve(fetched_value_id).provenance_chain[0]` == the http_response_received
   Event id AND a downstream sensitive-slot routing of that value Blocks (mirror
   `mint_from_read_anchor_identity`).
6. **SSRF resolve-and-pin (closes P7/P10):** `url` is I2-gated (routing-sensitive AND content-sensitive
   per §8/NIT-6). Resolve host → PIN the destination IP, connect to that pinned IP (SNI/Host = original
   host) to close DNS-rebind TOCTOU. DENY loopback (127/8, ::1), RFC1918 (10/8,172.16/12,192.168/16),
   link-local (169.254/16, fe80::/10), CGNAT (100.64/10), cloud-metadata (169.254.169.254), ULA (fc00::/7),
   IPv6-mapped (::ffff:0:0/96). NO redirect following by default. Reject userinfo@, non-https schemes,
   IP-encoding tricks (decimal/octal/hex). Host allowlist (operator-surfaced deployment constant) —
   never arbitrary. A non-allowlisted host or SSRF-range resolution → Deny.
7. **Crypto = `ring` (pure-Rust), CA roots = compiled-in `webpki-roots 1.0.8`** (§5.1/§5.2) so
   `env_clear()` is hermetic (no SSL_CERT_* / system store needed). (aws-lc-rs acceptable only if
   provider-consistency is materially cleaner — lean ring.)
8. **Gate-3 extension (§10 MANDATE):** in the SAME commit that adds `mint_from_http`, extend
   `scripts/check-invariants.sh` Gate 3 with a fifth `check_mint_token "mint_from_http("` restricted
   to the sanctioned loci (`quarantine.rs`, `server.rs`) — exactly as Phase 32 did for `mint_from_exec(`.
   Without it the new mint-site call-site restriction is silently unenforced.
</decisions>

<code_context>
## Existing Code Insights
- `crates/brokerd/src/sinks/email_smtp.rs` — Pattern A exemplar (in-broker egress, D-04 broker-only
  endpoint sourcing, opaque audit payloads).
- `crates/brokerd/src/quarantine.rs` — add `mint_from_http` beside `mint_from_read`(~301)/`mint_from_exec`(~838);
  reuse the event-first→mint→anchor + atomic in-conn demotion pattern (~316-401).
- `crates/runtime-core/src/plan_node.rs` — add `TaintLabel::HttpRaw` (compile-forced into is_untrusted ~45-56).
- `crates/executor/src/sink_schema.rs` + `sink_sensitivity.rs` — KNOWN_SINKS row for http.request; url
  routing+content-sensitive; Observe effect-class arm.
- `crates/brokerd/src/server.rs` — the mint_from_http call site (Allowed GET dispatch → fetch → mint → demote).
- `scripts/check-invariants.sh` Gate 3 (~134-137) — add the mint_from_http( token.
- New net dep wiring: reqwest rustls + ring + webpki-roots in brokerd's Cargo.toml (broker-side only).
- Tests: anti-staple (provenance root == event id + downstream Block); SSRF-range/userinfo/redirect denials
  (host-portable where possible via a resolver seam, Linux-gated where real sockets needed); non-allowlisted
  host Deny; session-demotes-on-response; HttpRaw untrusted. Live-HTTPS behavior → Phase 40.
</code_context>

<specifics>
## Specific Ideas
- SSRF resolve-and-pin should be structured so the IP-range/userinfo/scheme/redirect checks are
  host-portable unit-testable (a pure classifier over a resolved IP), with the real socket connect
  Linux-gated. This maximizes what's verifiable pre-Phase-40.
- Keep the allowlist a hardcoded/broker-config constant (NOT a policy file — I2/SSRF is a security property).
- No raw EffectRequest; check-invariants stays green (incl. the NEW Gate-3 token).
</specifics>

<deferred>
## Deferred Ideas
- POST/write egress → v1.9+. github.pr POST (Phase 38) reuses this GET egress infra + mint_from_http.
- Live-HTTPS env_clear/webpki-roots hermetic validation → Phase 40 (ENV-01, needs a real HTTPS run).
</deferred>
