---
phase: 52
slug: minimal-linux-packaging
status: verified
threats_open: 0
asvs_level: 1
block_on: high
created: 2026-08-11
register_authored_at_plan_time: true
threats_total: 13
threats_closed: 13
---

# Phase 52 — Security Verification

**Verdict: SECURED** — 13/13 declared threats closed (11 `mitigate`, 2 `accept`). ASVS L1; block-on `high`. No `high`-severity threat is in an `accept` disposition.

Mitigations were verified against the **current code on disk** after the 52-REVIEW.md fixes landed — not against the review's description of the earlier state. Verification combined grep/source reading with live adversarial execution of `scripts/install-linux.sh`, exceeding ASVS L1's minimum.

---

## Threat Verification

| Threat ID | Category | Severity | Disposition | Evidence |
|-----------|----------|----------|-------------|----------|
| T-52-01 | Elevation of Privilege | high | mitigate | `scripts/install-linux.sh:116` defaults to `${HOME}/.local/bin`; non-comment body scan for `sudo\|doas\|apt-get\|yum\|dnf\|pacman\|brew` = 0 matches; explicit no-escalation statement at line 135 |
| T-52-02 | Tampering (dest-path handling) | medium | mitigate | All `$DEST`/`$STAGE_DIR`/target-path expansions double-quoted; no `eval`/`source` of operator input; `mkdir -p` + `test -w` guard fires before build/stage (lines 129-137); live symlink-to-file attack at `${DEST}/caprun-worker` reproduced — `mv -f` via `rename(2)` replaces the symlink itself, victim file untouched |
| T-52-03 | Information Disclosure (credentials) | high | mitigate | `docs/CONFIGURATION.md:97,111-116` placeholder-only exports with least-scope framing; `grep -cE 'gh[pousr]_[A-Za-z0-9]{20,}'` across all four phase files = 0; broader token-shape scan (`sk-`, `AKIA`, `xox`) = 0; custody-boundary claim backed by real `env_clear()` at `crates/brokerd/src/sinks/process_exec.rs:503` and the `cli/caprun/src/main.rs` worker-spawn path |
| T-52-04 | Elevation of Privilege (do-not-set tier) | high | mitigate | `docs/CONFIGURATION.md:137-138` names `CAPRUN_ENABLE_IPC_CREATE_SESSION` (default-disabled gate, `crates/brokerd/src/server.rs:1767`) and `CAPRUN_CODING_I2_PROOF` (non-default `live-proof-fixtures` feature, `cli/caprun/Cargo.toml:8` + `main.rs:357-360`) |
| T-52-05 | Spoofing/Tampering (supply chain) | medium | mitigate | Non-comment body scan for `curl\|wget` = 0; `git diff --exit-code -- Cargo.toml Cargo.lock crates cli` clean (live-verified) |
| T-52-06 | Tampering (repo state) | medium | mitigate | Non-comment body scan for `git (add\|commit\|checkout\|reset\|clean\|push)` = 0 |
| T-52-07 | DoS (interrupted/concurrent install) | medium | **accept** | Documented residual in script header (`install-linux.sh:37-47`) and `docs/GETTING-STARTED.md:73`; surfaced to and accepted by the human at the Task 3 checkpoint. No lock protocol per RESEARCH "Don't Hand-Roll". See AR-52-01. |
| T-52-08 | Repudiation (no install-time audit record) | low | **accept** | `docs/GETTING-STARTED.md:91` explicitly disclaims "layout check, not a security proof"; no fabricated audit record. See AR-52-02. |
| T-52-09 | Tampering (config doc drift) | medium | mitigate | `## Operator Configuration Checklist` wholesale replacement confirmed; `SESSION_ID` "not read by any current code path" independently verified (`grep -rn SESSION_ID crates/ cli/` = 0); all 5 worker-protocol vars confirmed in source; bidirectional `CAPRUN_*` doc↔source check = 0 undocumented/unread names |
| T-52-10 | Elevation of Privilege (policy-file location) | medium | mitigate | `docs/CONFIGURATION.md:75` states the outside-workspace constraint; backed by `crates/brokerd/src/policy.rs` `bind_policy` + `refuse_if_beneath_workspace` and its test `bind_policy_refuses_path_beneath_workspace_root` |
| T-52-11 | Spoofing (false assurance) | high | mitigate | `docs/GETTING-STARTED.md:91` names `scripts/compose-verify.sh` as the authoritative harness |
| T-52-13 | Tampering (TCB/dependency surface) | high | mitigate | `git diff --exit-code -- Cargo.toml Cargo.lock crates cli` exit 0; `./scripts/check-invariants.sh` exit 0 (6/6 gates) |
| T-52-SC | Tampering (Cargo dependency surface) | medium | mitigate | Zero new packages — clean `Cargo.toml`/`Cargo.lock` diff; `git log` shows only `docs(52-*)`/`feat(52-01)`/`fix(52)` commits touching `scripts/`, `docs/`, `README.md` |

---

## Code-Review Fix Re-verification

All five 52-REVIEW.md findings were independently reproduced against the current code, not accepted from the review text:

- **WR-01** (`d8b840f`) — a pre-existing directory at `${DEST}/caprun-worker` now fails pre-flight with `FAIL — ... already exists as a directory`, exit 1, **before** build/stage begins. Previously printed `PASS` with the binary nested inside.
- **WR-02** (`7de6c4a`) — `--dest ""` now fails with `FAIL — --dest requires a non-empty directory argument`, exit 1. Previously installed silently to the default destination.
- **WR-03** (`7478ca2`) — `docs/GETTING-STARTED.md:75` now qualifies the manual-equivalent claim, stating the manual form lacks the script's staging/atomicity protection.
- **IN-01** (`a8308cf`) — `--dest` at an existing plain file now reports `... exists and is not a directory` instead of the misleading "not writable".
- **IN-02** (`a4cf155`) — a relative `--dest` invoked from a subdirectory resolves against the caller's `$PWD` (captured pre-`cd` as `CALLER_PWD`), not the repo root.

**No incompleteness or newly-introduced weakness was found in any of the five fixes.**

---

## Additional Adversarial Probe

Tested whether a pre-placed **symlink** at `${DEST}/caprun-worker` pointing at an arbitrary victim file could smuggle an attacker-controlled path into the sibling set or corrupt a file outside the destination.

**Result: safe.** Because `STAGE_DIR` is created *inside* `DEST` via `mktemp -d "${DEST}/..."`, the final `mv -f` is a same-filesystem `rename(2)`, which replaces the symlink itself rather than following it. The victim file was untouched and the destination path ended up holding the real binary as a regular file. No new threat ID required; this reinforces T-52-02.

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-52-01 | T-52-07 | Three POSIX renames are atomic individually, not as a set; concurrent installs into one destination are not serialized. Recovery: re-run the idempotent installer. No lock protocol built — explicitly out of scope per RESEARCH "Don't Hand-Roll". | Human, Task 3 checkpoint (`52-03-SUMMARY.md`) | 2026-08-11 |
| AR-52-02 | T-52-08 | The install-time layout check is deliberately non-destructive and produces no audit record; fabricating one would overstate what the check proves. The authoritative record remains the SQLite audit DAG / LIVE evidence. | Plan-time disposition, reaffirmed at Task 3 checkpoint | 2026-08-11 |
| AR-52-03 | Clean-machine install (backstop truth, 52-01/03) | Not performed — no EC2 host provisioned; dev box has no Docker and no passwordless sudo per CLAUDE.md. Human decided at the Task 3 checkpoint that this does not gate the milestone. | Human, Task 3 checkpoint | 2026-08-11 |
| AR-52-04 | Cold first-time-reader walkthrough (backstop truth, 52-03) | Not performed — no naive reader available. Human decided at the Task 3 checkpoint that this does not gate the milestone. | Human, Task 3 checkpoint | 2026-08-11 |

---

## Verification Boundary

The Linux security harness (`scripts/compose-verify.sh`, `scripts/mailpit-verify.sh`) was **not** run — both require Docker, unavailable on this host per CLAUDE.md. `cargo test --workspace` was deliberately **not** run either: CLAUDE.md warns a benign-looking run can trigger a LIVE SMTP send without a Mailpit listener.

This is a scope statement, not a gap: Phase 52 changed **zero** Rust code (`crates/`, `cli/`, `Cargo.toml`, `Cargo.lock` all untouched, live-verified), so no kernel-confinement behavior is in this phase's blast radius. Everything this phase did ship was verified locally by execution.

## Unregistered Flags

None. No `## Threat Flags` section exists in any of the three plan SUMMARY.md files, and review of the four implementation artifacts found no new attack surface outside the declared trust boundaries (operator shell → installer; source tree → installed binaries; docs → operator's machine).

**threats_open: 0**
