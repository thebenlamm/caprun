# DESIGN GATE RECORD — v1.9 (Authorized Egress + Policy & Audit Surface)

**Phase:** 41 — v1.9 DESIGN Gate + Fresh Adversarial Code-Trace
**DESIGN doc under review:** `planning-docs/DESIGN-v1.9-egress-policy.md`
**Requirements gated:** DESIGN-17 (pin the 3 TCB mechanisms), DESIGN-18 (clear a fresh non-self adversarial code-trace before any TCB code)
**Status:** ✅ **CLEARED** (Round-1 amendments) — Phases 42–46 TCB code authorized.
**Date:** 2026-07-18

## Gate discipline (standing precedent, unbroken v1.0 P2 → v1.8 P35)

No `crates/{executor,brokerd,sandbox,runtime-core}` or `cli/` code may be written until this
DESIGN doc clears a **fresh, non-self, ORCHESTRATOR-owned** adversarial code-trace. The
orchestrator (not a gsd-executor — gsd-executors have no Agent tool) owns the review spawn and
the finding-fold. The doc was authored by a gsd-executor; the review was spawned by the
orchestrator against a genuinely fresh reviewer with no authoring involvement.

## Review 1 — fresh non-self adversarial code-trace (Fable-5)

The reviewer traced every load-bearing `file:line` citation in the doc against current source
(`http_request.rs`, `seccomp.rs`, `process_exec.rs`, `key.rs`, `confirmation.rs`,
`sink_sensitivity.rs`, `lib.rs`, `sink_schema.rs`, `check-invariants.sh`). It confirmed: the
git.push broker-performed smart-HTTP model (§1) is sound and correctly grounded (child stays
net-denied per `exec_child_filter` `seccomp.rs:147,163-188`; pin is application-layer only,
honoring v1.8 BLOCKER-1); the research pin (candidate b) and the v1.8 §2.5/§2.7/§9
carry-forwards are faithful; the policy↔I2 order (§5.1/§5.2) is realizable as a deny-only
pre-I2 gate keeping I2 unconditional.

**Verdict: NOT CLEARED — 1 BLOCKER-level MAJOR + 1 MAJOR + 3 MINOR.** All findings were
independently re-verified against live code by the orchestrator before folding (no false
positives — both MAJORs confirmed true):

| # | Sev | Finding | Confirmed code fact | Resolution |
|---|-----|---------|---------------------|------------|
| MAJOR-1 | BLOCKER-level | `http.request` WRITE had no pinned effect class → defaults to `Observe` → a POST from a draft/untrusted-seeded session would escape the I0 gate. | `sink_effect_class` keys on sink-id only (`sink_sensitivity.rs:40-83`), `"http.request" => Observe` (:64); I0/Draft deny fires ONLY for `CommitIrreversible` (`lib.rs:217`); `_ =>` default is CommitIrreversible (:83, so a distinct id fails closed). | **Fixed §2.0/§0/§4/§7/§8:** pin a **distinct sink id `http.request.write`** classed **`CommitIrreversible`** (I0 draft-deny + I2 + confirm-releasable), never an extension of the Observe GET id. §7's I0 checkbox now CITES the class. |
| MAJOR-2 | MAJOR | POLICY-03's "reuse F1 containment verbatim from `key.rs`" is not factorable — the check is inline in `load_or_create_key` and `pub(crate)` in the caprun CLI crate, unreachable from the broker binder. | `key.rs:60-110` inline F1 (`starts_with` refuse :88-95, `canonicalize_existing_or_parent` :166); no standalone helper; `pub(crate)` in `cli/caprun`, POLICY-03 binder is in `brokerd`. | **Fixed §5.3/§0/§8:** mandate **extracting** a shared, unit-tested `refuse_if_beneath_workspace(path, root)` into a crate reachable by BOTH sites (note cross-crate lift → runtime-core/shared util); both call it; regression test asserts it; preserves the fail-closed-on-unresolvable semantics. |
| MINOR-3 | MINOR | `invoke_pinned_post` re-resolves DNS (`:531`→`:435-444`), breaking §1.5's frozen-IP freeze if reused. | Confirmed. | **Fixed §1.4/§1.5:** `invoke_pinned_post` NOT reusable as-is; Phase 44 builds one `reqwest::Client` from a single vetted `SocketAddr` for BOTH the info/refs GET and receive-pack POST. |
| MINOR-4 | MINOR | Credential/URL-absence assertion omitted broker LOG output (a research-named leak vector). | `do_pinned_post` error path `http_request.rs:542` can log URL/redirect material. | **Fixed §1.4/§10:** assertion extended to broker LOG output on the git-push HTTP legs (flows to LIVE-06 leg 5). |
| MINOR-5 | MINOR | `send-pack --stateless-rpc` "no socket" stated high-confidence vs research MEDIUM; §2 silent on `method` validation. | Research rates it MEDIUM. | **Fixed §1.1/§2.6:** broker-generates-receive-pack-body-directly is now the PRIMARY realization (stateless-rpc = documented alternative + §1.8 seccomp backstop); `method` pinned as a schema-validated `{POST,PUT}` enum, allowlist/class selection keys off the validated method. |

## Round-1 amendments — orchestrator re-verification

The fold (executor commit `7653464`, docs-only, +212/−51) was verified by the orchestrator:
- **MAJOR-1** — §2.0 now pins `http.request.write` ⇒ `CommitIrreversible` (explicit row + fail-closed `_ =>` default) with the exact I0-escape rationale; §0/§4/§7/§8 reconciled. Closes the finding against `sink_sensitivity.rs:64` / `lib.rs:217`.
- **MAJOR-2** — §5.3 now mandates EXTRACTING the F1 predicate into a shared helper both sites call (+ regression test), capturing the cross-crate `pub(crate)` unreachability. Closes the finding against `key.rs:60-110`.
- MINOR-3/4/5 folded as described; every prior sound section left intact; changelog section added to the doc.
- Docs-only invariant held throughout (`git status --porcelain -- crates cli` empty; `check-invariants.sh` exit 0).

Both MAJORs are fail-closed pins (distinct sink id → CommitIrreversible; extract-and-test a
containment helper) whose closure was confirmed against the actual source, so the gate clears on
Round 1 — matching the v1.5/v1.6 Round-1-clears precedent. The doc's §9 mandates that this
adversarial trace **re-runs if the git.push trust-posture or transport-dependency choice changes
mid-implementation** (Phase 44), and the standing per-phase TCB-diff adversarial-trace discipline
(Phases 42–46) is the next guardrail layer.

## Outcome

- **DESIGN-17:** ✅ the doc pins all 3 TCB mechanisms (git.push broker-performed egress, http.request WRITE, policy↔I2 boundary incl. POLICY-03), carries forward v1.8 §2/§2.5/§2.7/§9, ring-only supply chain, fail-closed defaults, §-per-pitfall threat model.
- **DESIGN-18:** ✅ cleared a fresh non-self orchestrator-owned adversarial code-trace (1 BLOCKER-level MAJOR + 1 MAJOR + 3 MINOR, all folded Round-1 and orchestrator-re-verified against live code).
- **Gate:** ✅ **CLEARED.** Phases 42–46 TCB code authorized. No `crates/`/`cli/` code was written during Phase 41.

*The fresh-context adversarial code-trace earned its keep again (~10th real catch): a passing
plan-checker + a green docs-only invariant both missed a BLOCKER-level I0-gate escape that the
code-tracing review caught before a line of implementation existed.*
