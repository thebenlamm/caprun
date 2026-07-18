# Requirements: caprun (AgentOS) — v1.9 Authorized Egress + Policy & Audit Surface

**Defined:** 2026-07-18
**Core Value:** A kernel-confined worker can only cause external effects through broker-mediated plan nodes, and a genuinely-propagated taint chain deterministically blocks value-injection at the sink (I2) — v1.9 completes the authorized-write-egress loop (edit→test→commit→push→open-PR) and adds the first trust-surface layer (policy + CLI/audit-viewer), without weakening I0/I1/I2 or adding any raw `EffectRequest` path.

> **Requirements hardened 2026-07-18 by two independent reviews** (matt-essentialist right-sizing + a fresh non-self Fable-5 adversarial code-trace). Both converged on the #1 gap — policy provenance/binding (now POLICY-03). Fable-5 verified its findings against live code (`cli/caprun/src/key.rs` F1 custody, `HOST_ALLOWLIST` in `http_request.rs`, `check-invariants.sh` Gate 5). Folded findings are annotated `[rev: …]`.

## v1 Requirements

Requirements for the v1.9 milestone. Each maps to exactly one roadmap phase (see Traceability). REQ-IDs continue from v1.8 (DESIGN-16, GIT-01, HTTP-03, GITHUB-04, ENV-01, LIVE-04).

### Design Gate (blocks all TCB code)

- [x] **DESIGN-17**: A single DESIGN doc pins the TCB mechanisms for v1.9 — (a) the fully-unprivileged, broker-mediated, destination-pinned `git.push` egress (child net-denied; the pin lives in a broker/netfilter/application layer that can SEE the destination, NEVER in seccomp — the research-recommended mechanism is a **broker-performed git smart-HTTP transfer** reusing the shipped reqwest+rustls(ring)+webpki-roots+SSRF resolve-and-pin stack, child does only local pack generation; see `.planning/research/GIT-PUSH-EGRESS.md`); (b) the `http.request` WRITE (POST/PUT) egress; (c) the **policy-vs-I2 boundary** — exactly what policy can and cannot do, AND where policy comes from / how it binds (POLICY-03). Carries forward v1.8 §2/§2.5(credential scrub)/§2.7(payload-at-confirm)/§9(confirm-release). If no fully-unprivileged destination-pinning mechanism proves sound, the doc formalizes deferring `git.push` (the other tracks proceed).
- [x] **DESIGN-18**: The DESIGN doc clears a fresh, non-self, orchestrator-owned adversarial code-trace (NOT a gsd-executor) before any `crates/{executor,brokerd,sandbox,runtime-core}` TCB code — unbroken precedent through v1.8 P35. `[rev: n2]` The trace **re-runs if the git.push trust-posture or transport-dependency choice changes mid-implementation** — the deferral doc itself calls git.push "the riskiest surface in the project," so a mid-build transport pivot must not bypass the one gate meant to catch it.

### Authorized Egress — git.push

- [ ] **GIT-02**: `git.push` sink — a fully-unprivileged, broker-mediated, destination-pinned egress with the push child kept net-denied (no seccomp relaxation). The remote URL + refspec are captured from TRUSTED intent, never the untrusted repo's `.git/config`. `--force`/`--force-with-lease`/ref-deletion/`+`-force-refspec are hard-denied by construction (unreachable even via human confirm). The push credential lives in broker-local env only (never a ValueNode/plan-arg/audit-literal/the child/planner) and is never followed across a `receive-pack` redirect (`[rev: research attack-point]`). Captured child/transport output is scrubbed of any credential/URL material (or not minted at all) before it can reach the value store or audit chain (§2.5). **Safety-valve:** if the design gate proves no sound unprivileged mechanism exists (research currently assesses one FEASIBLE), GIT-02 defers rather than shipping arbitrary child egress — a disclosed, sign-off-gated deferral, never a silent drop (see LIVE-05).
- [ ] **GIT-03**: A tainted push `remote`/`refspec` deterministically Blocks at the sink under I2 and is releasable only by single-shot human confirmation, whose terminal audit event is written before the terminal state (P33/P34 `prepare_git_push` precheck discipline). At the confirm prompt the human is shown the pushed payload — commit range/branch + a provenance summary flagging any pushed file whose content derives from untrusted taint (§2.7) — and the pack pushed is generated from that confirmed commit range at-or-after confirm (no payload-vs-destination confirm TOCTOU, `[rev: research attack-point]`), not just the destination. (Defers with GIT-02.)

### Authorized Egress — http.request WRITE

- [x] **HTTP-W-01** *(sink + differential proven at the decision+dispatch boundary in Phase 43; the "clean leg actually delivered to a live mock endpoint (mock records receipt)" sub-clause is by-design carried to Phase 46 LIVE-05/06 — Phase 43 ships `WRITE_HOST_ALLOWLIST` empty/fail-closed, so no live write mock exists until the composed proof)*: `http.request` WRITE (POST/PUT) to an allowlisted host. The request BODY is taint-governed and content-sensitive under I2 (a tainted body deterministically Blocks, exactly like an email/PR body); the `url` is routing-sensitive. Reuses v1.8's SSRF resolve-and-pin (loopback/RFC1918/link-local/metadata/userinfo@/redirect denied) + webpki-roots egress. `[rev: M1]` Any write credential lives in broker-local env only (never a ValueNode/plan-arg/audit-literal/the worker/planner), and the captured response is scrubbed of credential material (or not minted) before value-store/audit. `[rev: m1]` The WRITE (mutating) host-allowlist is **distinct** from the read/GET allowlist — a host being GET-readable does not imply it is POST/PUT-writable. `[rev: M4]` Acceptance is **differential**: the tainted-body-Blocks leg and the clean-body-Allowed leg are identical in host/url/method/policy (taint is the sole variable), and the clean leg is confirmed to have actually delivered the body to the mock endpoint on real Linux (mock records receipt) — not merely "not blocked," so a block-everything I2 regression cannot pass.

### Policy (which-sinks-callable only — NEVER overrides I2)

- [x] **POLICY-01**: A minimal declarative per-session policy — a hardcoded-schema struct/file (NOT Cedar) specifying which sinks are callable + coarse arg constraints (allowlisted hosts/paths/repos). A sink or arg not permitted by the session's policy is refused with a **distinct, machine-checkable policy-deny outcome** (separate from an I2 Block).
- [x] **POLICY-02** (LOCKED INVARIANT): Policy may only gate WHICH sinks/args are callable — it can NEVER disable or override I2. An attacker-tainted value in a sensitive sink arg still Blocks regardless of policy; the I2 decision stays HARDCODED in the Rust TCB executor (DEC/CON-i2-non-bypassable). `[rev: m3]` I2 executes unconditionally on every policy-**permitted** call and can never be short-circuited by any policy outcome (policy is a pre-I2 narrowing gate, never a post-I2 override). Proven by a live leg where a permissive policy does NOT weaken the I2 taint Block.
- [x] **POLICY-03** `[rev: B1 + Matt #1 — BLOCKER, both reviewers converged]`: The session policy is **bound by the broker at session creation from a trusted source provably outside the confined worker's reach** — canonicalized and refused if it resolves at-or-beneath the workspace root (reuse the F1 containment check from `key.rs` verbatim), immutable for the session's life, with its identity/hash recorded as a genuine audit-DAG event. A confined worker cannot mutate its own policy. Proven by a negative live leg: a worker that writes/rewrites a policy file mid-session does NOT change the enforced allowlist.

### Trust Surface — CLI/SDK + Audit-DAG Viewer

- [ ] **SDK-01**: A thin CLI/SDK to define an intent, point it at a workspace, and run it end-to-end against the broker — manual-ops-first, no framework (extends the existing `caprun confirm`/`deny`/`grant`/`review` verbs, does not replace them). `[rev: Matt #3]` The run entrypoint takes the trusted policy path (POLICY-03) and binds it at session creation — this is the enforcement point that connects Track 3 to Track 4. `[rev: Matt #2]` When a sink Blocks under I2, the entrypoint surfaces the blocked `effect_id` (+ the `caprun review` pointer) so the operator can reach the existing confirm/deny/grant verbs — the sub-capability that makes the loop actually design-partner-runnable. `[rev: M7]` SDK-constructed values carry trusted provenance ONLY for genuinely operator-typed literals; any file-/stream-/env-sourced content the SDK ingests is minted TAINTED (draft-only per I0/I1), exactly like any other raw read — the SDK is not a provenance-laundering path.
- [ ] **U1** (VIEW-01): A read-only audit-DAG viewer over the SQLite audit chain that renders a session's events/decisions and surfaces `verify_chain` — the trust surface a design partner uses to INSPECT the proof. No web UI. `[rev: M2]` The viewer reuses the exact `load_or_create_key` MAC-key custody + F1 containment refusal, fails closed (refuses to render a `verify_chain` verdict) if the key is absent, and never loads a fresh/`:memory:` key (which would make the verdict meaningless); it must be out of the confined worker's reach. `[rev: M3]` All tainted literal bytes (e.g. a tainted commit message or POST body) are control-char-neutralized/escaped before display — a terminal viewer must never interpret attacker-tainted content as ANSI/formatting (audit-line spoofing surface).

### Supply-Chain & Invariant Hygiene

- [ ] **HYG-01** `[rev: Matt #6 + m2]`: An automated, workspace-scoped supply-chain **absence assertion** — prove zero new forbidden C-crypto dependency crept in (`cargo tree --workspace -i <dep>` = absent for aws-lc-rs/openssl-sys; ring-only + webpki-roots), the resolver-3 feature-unification lesson. The check **re-runs after the git.push transport dependency is chosen**, enumerating any new transport deps (not just deps known at planning time); if a new dep IS added it must honor the ring-only recipe. Folds in the two v1.8-adversarial-trace-flagged hygiene items: a feature-OFF guard step in `compose-verify.sh` and broadening `check-invariants` Gate 4b to a workspace-wide grep.

### Live Proof (v1.9 DONE gate)

- [ ] **LIVE-05**: A composed workflow — `process.exec` (test) → filesystem edit → `git.commit` → `git.push` → `github.pr` PLUS an `http.request` POST leg — runs on real Linux (mock git remote + mock endpoint), DRIVEN and INSPECTED via the new CLI + audit-DAG viewer (SDK-01/U1 are on the acceptance critical path, not trailing tooling), with every step gated/tainted/audit-DAG-chained and `verify_chain` true across the run. `[rev: M6/n1]` If GIT-02 defers, the `git.push` leg auto-descopes AND the deferral is recorded as a disclosed milestone gap requiring explicit user sign-off — never an orchestrator-autonomous silent drop.
- [ ] **LIVE-06**: In the same proof, adversarial/negative legs each deterministically Block/refuse, **each independently attributable**: (1) a tainted push remote/refspec (I2 Blocks); (2) a tainted POST body (I2 Blocks); (3) `[rev: M5]` a policy-deny leg (an off-allowlist sink refused via the distinct policy-deny outcome) — where the I2-Block legs run a sink+arg the policy explicitly PERMITS, so policy is provably not what's blocking, and the two mechanisms emit distinct machine-checkable terminal-event tags asserted separately; (4) `[rev: M8]` a destination-pin negative — a push/POST whose destination is redirected/off-pin is refused at the broker/application layer (proves the pin holds, not just that a happy push reaches a listener); (5) `[rev: Matt #4]` a credential-absence assertion — after a real push, no credential or remote-URL material appears in the value store or audit chain. Full-workspace regression green on real Linux, no v1.0–v1.8 regression.

## v2 Requirements

Deferred to a future release. Tracked, not in the v1.9 roadmap.

### Effect Breadth / Productization

- **GITHUB-05**: github.pr merge/comment breadth.
- **PLANNER-05**: the real (multi-step tool-use) LLM planner loop on the v1.4 sidecar seam (planner stays deterministic/stub in v1.9 — no AI eval set this milestone).
- **PKG-01**: packaging / install story.
- **VIEW-02**: audit-DAG viewer also renders live pending/blocked state ("what's waiting on you right now"), subsuming `caprun review` — additive; defer unless a design partner asks (Matt "worth considering").

## Out of Scope

Explicitly excluded from v1.9. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Cedar policy engine | A hardcoded-schema policy struct/file suffices (POLICY-01); I2 stays in the Rust TCB, never a swappable policy file (DEC/CON-i2-non-bypassable). |
| Policy that can disable/override I2 | LOCKED anti-requirement — the #1 adversarial-trace risk; I2 is non-bypassable regardless of policy (POLICY-02/03). |
| Policy loaded from a session/worker-writable location | LOCKED anti-requirement — a worker could widen its own allowlist (the F1-precedent attack); policy is bound from a trusted source outside worker reach (POLICY-03). |
| Real LLM planner loop (multi-step tool-use) | Planner stays deterministic/stub in v1.9 → no AI eval set needed this milestone. Deferred to v2. |
| github.pr merge/comment breadth | Out of the authorized-egress + trust-surface theme; deferred to v2. |
| Web UI | The read-only CLI/audit-DAG viewer (U1) is the trust surface; a web UI is out of scope. |
| Cross-host delegation / Biscuit crypto | v3 concern, unchanged. |
| Seccomp-based git.push destination pinning | Provably impossible at the kernel floor (v1.8 BLOCKER-1): seccomp filters syscall numbers/scalars, not the `connect()` sockaddr behind a pointer; Landlock net needs kernel 6.7 > the 5.13 floor. The pin MUST live in a broker/netfilter/application layer. |
| Net-allowed git.push child | Would grant arbitrary egress to a credential-bearing child — the exact exfil primitive the taint model defeats. Child stays net-denied. |
| pasta/slirp4netns + netns egress filter for git.push | Rejected by research: un-gated external C binary, largest new kernel attack surface (userns + netfilter CVEs), host-policy-gated (unprivileged userns often disabled), hardest to live-prove. Broker-performed smart-HTTP transfer (candidate b) is preferred. |
| Any raw `EffectRequest{args}`→sink path | Architecturally locked out since v0 (check-invariants Gate 1); every effect is a plan node. |

## Traceability

Which phases cover which requirements. Populated during roadmap creation (`/gsd-roadmapper`, 2026-07-18). Phase numbering CONTINUES from v1.8 (last phase = 40); v1.9 spans Phases 41-46.

| Requirement | Phase | Status |
|-------------|-------|--------|
| DESIGN-17 | Phase 41 | Complete |
| DESIGN-18 | Phase 41 | Complete |
| POLICY-01 | Phase 42 | Complete |
| POLICY-02 | Phase 42 | Complete |
| POLICY-03 | Phase 42 | Complete |
| HTTP-W-01 | Phase 43 | Complete (sink + differential proven P43; live mock-receipt → P46 LIVE-05/06) |
| GIT-02 | Phase 44 | Pending |
| GIT-03 | Phase 44 | Pending |
| HYG-01 | Phase 44 | Pending |
| SDK-01 | Phase 45 | Pending |
| U1 | Phase 45 | Pending |
| LIVE-05 | Phase 46 | Pending |
| LIVE-06 | Phase 46 | Pending |

**Coverage:**

- v1 requirements: 13 total
- Mapped to phases: 13 ✓ (Phases 41-46, 6 phases)
- Unmapped: 0 ✓ (no orphans, no duplicates)

**Phase → requirement rollup:**

- **Phase 41** (DESIGN gate): DESIGN-17, DESIGN-18
- **Phase 42** (Policy layer): POLICY-01, POLICY-02, POLICY-03
- **Phase 43** (http-write egress): HTTP-W-01
- **Phase 44** (git.push egress + hygiene): GIT-02, GIT-03, HYG-01
- **Phase 45** (CLI/SDK + audit-DAG viewer): SDK-01, U1
- **Phase 46** (composed live proof — v1.9 DONE): LIVE-05, LIVE-06

**Sequencing (both reviewers agree):** DESIGN-17/18 gate FIRST (blocks all TCB code) → POLICY-01 + POLICY-03 binding as the foundation, POLICY-02 invariant enforced with them (sinks get gated by it; live legs need it) → GIT-02/03 ∥ HTTP-W-01 (shared already-shipped transport stack, split by blast radius so a git.push deferral leaves http-write untouched) → SDK-01 + U1 (the driver+inspector LIVE-05 requires — critical path, not trailing tooling) → LIVE-05/06 (composed proof; POLICY-02's non-bypass and the 5 negative legs re-demonstrated live).

---
*Requirements defined: 2026-07-18*
*Last updated: 2026-07-18 — hardened by matt-essentialist + Fable-5 adversarial reviews (POLICY-03 added, 12→13 reqs; credential-custody/viewer-hardening/differential-proof folds); traceability populated by `/gsd-roadmapper` (13/13 mapped to Phases 41-46, 0 orphans).*
