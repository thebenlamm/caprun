---
phase: 40-cli-compose-env-clear-live-proof
plan: 02
subsystem: infra
tags: [rustls, webpki-roots, tls, egress, ssrf, cargo-features, brokerd, http.request, github.pr]

# Dependency graph
requires:
  - phase: 37-http-request-egress
    provides: ring/webpki-roots egress TLS client + validate_url/ssrf_check/resolve-and-pin
  - phase: 38-github-pr-sink
    provides: invoke_pinned_post + HOST_ALLOWLIST = [api.github.com]
provides:
  - "Non-default cargo feature `mock-egress-ca` on crates/brokerd"
  - "Feature-gated egress trust anchor (one checked-in test CA) via egress_root_store()"
  - "Feature-gated egress allowlist host github-mock.caprun.test"
  - "Default-build guard tests proving release trust = webpki-roots only + allowlist = [api.github.com] only"
  - "check-invariants Gate 4b: mock-egress-ca can never be a default feature"
affects: [40-03, 40-04, mailpit-verify, compose-live-proof]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Non-default cargo feature gating test-only egress trust additions (mirrors test-fixtures)"
    - "Extract trust-set builder (egress_root_store) so the RootCertStore is directly unit-testable"
    - "Feature-OFF invariant tests gated #[cfg(not(feature=...))] + a check-invariants gate forbidding the feature as default (belt + suspenders against false-assurance self-disable)"

key-files:
  created:
    - crates/brokerd/tests/fixtures/mock-egress-ca.der
    - crates/brokerd/tests/fixtures/README-mock-egress-ca.md
  modified:
    - crates/brokerd/Cargo.toml
    - crates/brokerd/src/sinks/http_request.rs
    - scripts/check-invariants.sh

key-decisions:
  - "HOST_ALLOWLIST const left byte-for-byte [api.github.com]; the mock host is added ONLY inside is_host_allowlisted under the feature gate, so the release allowlist literal is provably unchanged."
  - "Feature adds ONLY one trust anchor + one host; validate_url/ssrf_check/vet_resolved/resolve_and_pin/build_pinned_client/do_pinned_*/invoke_* are untouched (grep-verified)."
  - "Rule 2 deviation: added check-invariants Gate 4b forbidding mock-egress-ca as a default, because the not(feature)-gated release-trust guards would silently compile OUT (not fail) if the feature ever became default — the HARDEN-04 false-assurance class."

patterns-established:
  - "Test-only egress trust extension behind a never-default cargo feature, enforced by both a default-build assertion test AND a check-invariants never-default gate."

requirements-completed: [LIVE-03]

coverage:
  - id: D1
    description: "Non-default cargo feature mock-egress-ca declared on crates/brokerd; never a default, not in the self dev-dependency."
    requirement: "LIVE-03"
    verification:
      - kind: other
        ref: "cargo build --workspace (feature off) + cargo build -p brokerd --features mock-egress-ca; awk/grep assert not in default feature set nor self dev-dep"
        status: pass
      - kind: unit
        ref: "scripts/check-invariants.sh Gate 4b (mock-egress-ca not a default feature)"
        status: pass
    human_judgment: false
  - id: D2
    description: "RELEASE/default build egress trust set = webpki-roots ONLY and allowlist = [api.github.com] ONLY (mock CA + mock host unreachable without the feature)."
    requirement: "LIVE-03"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/sinks/http_request.rs#egress_root_store_default_build_is_webpki_roots_only"
        status: pass
      - kind: unit
        ref: "crates/brokerd/src/sinks/http_request.rs#allowlist_default_build_is_api_github_only"
        status: pass
    human_judgment: false
  - id: D3
    description: "Feature ON adds EXACTLY one trust anchor (webpki + 1) + one allowlisted host (github-mock.caprun.test), still allowlisting api.github.com; SSRF pin/validate_url untouched."
    requirement: "LIVE-03"
    verification:
      - kind: unit
        ref: "crates/brokerd/src/sinks/http_request.rs#mock_egress_ca_feature_adds_exactly_one_host_and_one_anchor (run --features mock-egress-ca)"
        status: pass
      - kind: other
        ref: "git diff shows no edits to validate_url/ssrf_check/vet_resolved/resolve_and_pin/build_pinned_client/do_pinned_*/invoke_*"
        status: pass
    human_judgment: false

# Metrics
duration: 7min
completed: 2026-07-18
status: complete
---

# Phase 40 Plan 02: Non-default `mock-egress-ca` egress trust feature Summary

**A never-default cargo feature that adds EXACTLY one checked-in test CA + one test host (`github-mock.caprun.test`) to the broker egress under gate — with the release build provably webpki-roots-only + `[api.github.com]`-only, and the SSRF resolve-and-pin path untouched.**

## Performance

- **Duration:** ~7 min
- **Started:** 2026-07-18T10:13:40Z
- **Completed:** 2026-07-18T10:20:56Z
- **Tasks:** 2
- **Files modified:** 5 (2 created, 3 modified)

## Accomplishments
- Added the non-default `mock-egress-ca` cargo feature to `crates/brokerd` (mirrors the never-default `test-fixtures` discipline; absent from every default/production build and NOT enabled via the self dev-dependency).
- Checked in `tests/fixtures/mock-egress-ca.der` — a self-signed test CA for `github-mock.caprun.test` (CA:TRUE, SAN DNS, no production trust) + a README documenting the offline `openssl` reproduction command (no Rust cert-gen dependency added).
- Extracted `egress_root_store()` and feature-gated a single `roots.add(CertificateDer)` of the embedded DER; feature-gated the single extra host in `is_host_allowlisted` while leaving `HOST_ALLOWLIST` byte-for-byte `["api.github.com"]`.
- Proved the default build unchanged via two guard tests + proved the feature-ON delta is exactly +1 anchor / +1 host; `validate_url`, `ssrf_check`, `vet_resolved`, `resolve_and_pin`, `build_pinned_client`, `do_pinned_*`, and `invoke_*` are grep-verified untouched.

## Task Commits

1. **Task 1: test CA fixture + non-default feature** - `42728c4` (feat)
2. **Task 2 (RED): trust-set + allowlist invariant tests** - `17846fc` (test)
3. **Task 2 (GREEN): feature-gated anchor + host + Gate 4b** - `c6ade16` (feat)

_TDD: RED (`17846fc`, feature-ON test fails on unbuilt behavior) → GREEN (`c6ade16`)._

## Files Created/Modified
- `crates/brokerd/tests/fixtures/mock-egress-ca.der` - self-signed test CA trust anchor (test-only, no production trust).
- `crates/brokerd/tests/fixtures/README-mock-egress-ca.md` - reproduction command + security note.
- `crates/brokerd/Cargo.toml` - `mock-egress-ca = []` non-default feature.
- `crates/brokerd/src/sinks/http_request.rs` - `egress_root_store()` extraction + feature-gated anchor + feature-gated host + guard tests + module doc.
- `scripts/check-invariants.sh` - Gate 4b forbidding `mock-egress-ca` as a default feature.

## Real Verification Results
- `cargo build --workspace` (feature OFF): **Finished, OK.**
- `cargo build -p brokerd --features mock-egress-ca`: **Finished, OK.**
- `cargo test -p brokerd` (default): **180 lib + all integration tests passed, 0 failed.**
- Named guard tests (feature OFF): `egress_root_store_default_build_is_webpki_roots_only` **ok**, `allowlist_default_build_is_api_github_only` **ok** (2 passed).
- Feature-ON test: `mock_egress_ca_feature_adds_exactly_one_host_and_one_anchor` **ok** (root count = webpki + 1; observed 118 → 119).
- `./scripts/check-invariants.sh`: **EXIT=0, all gates PASSED** (incl. new Gate 4b, Gate 5 ring-only / no aws-lc-rs).
- No raw `EffectRequest` token; no file deletions; no untracked files.

**Trust-set confirmation:** with `mock-egress-ca` OFF (the release/default build), the egress trust set is `webpki-roots` ONLY (`egress_root_store().roots.len() == webpki_roots::TLS_SERVER_ROOTS.len()`) and the host allowlist is `[api.github.com]` ONLY (`github-mock.caprun.test` is NOT allowlisted, the test CA is absent). The mock CA + mock host are unreachable unless the non-default feature is compiled in.

## Decisions Made
- Kept `HOST_ALLOWLIST` const literally `["api.github.com"]` and added the mock host only inside `is_host_allowlisted` behind the gate — so the release allowlist literal is provably unchanged and the guard test asserts against the const directly.
- Gated the feature-OFF invariant tests with `#[cfg(not(feature = "mock-egress-ca"))]` (they assert the feature-OFF invariant, which does not hold once the feature is compiled in — this was caught during RED when the count test failed 119 vs 118 under `--features`).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added check-invariants Gate 4b forbidding `mock-egress-ca` as a default feature**
- **Found during:** Task 2 (guard-test design)
- **Issue:** The release-trust guard tests are gated `#[cfg(not(feature = "mock-egress-ca"))]`. If `mock-egress-ca` were ever made a `default` feature, those guards would silently compile OUT rather than fail — the exact HARDEN-04 / false-assurance-regression class the existing Gate 4 protects `test-fixtures` against. The plan's own key-link ("mirror the existing test-fixtures feature pattern") points at this discipline.
- **Fix:** Added Gate 4b to `scripts/check-invariants.sh` asserting `mock-egress-ca` is never inside brokerd's `[features] default = [...]` list — belt to the guard test's suspenders.
- **Files modified:** scripts/check-invariants.sh
- **Verification:** `./scripts/check-invariants.sh` EXIT=0, "Gate 4b ... PASS".
- **Committed in:** `c6ade16` (Task 2 GREEN commit)

---

**Total deviations:** 1 auto-fixed (1 missing-critical / security).
**Impact on plan:** Strengthens the T-40-03 mitigation; no scope creep (`scripts/check-invariants.sh` is disjoint from Plan 40-01's `cli/caprun/src/main.rs`). All other work followed the plan exactly.

## Issues Encountered
- During RED under `--features mock-egress-ca`, the two "default-build" invariant tests failed (119 vs 118 anchors). Root cause: they assert the feature-OFF invariant, which is false when the feature is compiled in. Resolved by gating them `#[cfg(not(feature = "mock-egress-ca"))]`.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Plan 40-03 can enable `--features mock-egress-ca` in the compose-verify harness so the `github.pr` POST reaches a local TLS mock (`github-mock.caprun.test` on the 203.0.113.0/24 public-range docker subnet, which passes `ssrf_check` unmodified) over real TLS while riding the shipped pin path.
- The matching PEM cert + private key for the mock server are NOT in this plan (per plan) — Plan 40-03 supplies them to the mock GitHub endpoint.
- SECURITY FLAG for the closing fresh adversarial code-trace (T-40-03): confirm the release (no-feature) trust set is webpki-roots only and allowlist is [api.github.com] only — enforced here by `egress_root_store_default_build_is_webpki_roots_only`, `allowlist_default_build_is_api_github_only`, and Gate 4b.

## Self-Check: PASSED
All created/modified files present; all task commits (`42728c4`, `17846fc`, `c6ade16`) exist in git.

---
*Phase: 40-cli-compose-env-clear-live-proof*
*Completed: 2026-07-18*
