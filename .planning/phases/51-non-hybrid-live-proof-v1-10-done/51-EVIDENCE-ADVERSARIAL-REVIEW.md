# Phase 51 Real-Linux Evidence — Independent Adversarial Review

> Read-only review of Phase 51's retained real-Linux proof, performed against
> `51-EVIDENCE-ADVERSARIAL-REVIEW-PROMPT.md` before human approval of Plan 51-04.
> Review date: 2026-08-09. Repository HEAD at review time: `43fdb67`.

## Reviewer identity and independence

| Field | Value |
|---|---|
| Reviewer kind | Fresh-context, read-only adversarial reviewer |
| Model/runtime | Claude Code, Opus 5 (1M context) |
| Independence | Did not author Plan 51-04, Plan 51-09, the test oracle, the evidence record, or commits `43eb822`/`43fdb67`. Assumed no prior claim correct. |
| Mutations | **None.** No file edited, generated, deleted, staged, or committed during the review. Working tree at end of review was byte-identical to its state at start (`M .planning/STATE.md`, `?? …51-EVIDENCE-ADVERSARIAL-REVIEW-PROMPT.md`). This report file was written afterwards, on explicit instruction. |
| External access | **No AWS query performed.** No credentials used; see "Teardown verification limits". |

## Files opened and commands run

Opened: `51-04-PLAN.md`, `51-09-PLAN.md` (via `51-09-SUMMARY.md`), `51-09-SUMMARY.md`,
`51-LIVE-EVIDENCE.md`, `51-LIVE-SCOPED.log` (all 519 lines), `51-LIVE-FULL.log`
(targeted + tail), `51-LIVE-SCOPED-FAIL-PRIOR-9b78bd4f.log` (tail), `51-VALIDATION.md`,
`51-ADVERSARIAL-TRACE.md`, `.planning/REQUIREMENTS.md` (diff), `.planning/WINDOWS.md`
(diff), `.planning/ROADMAP.md`, `.planning/STATE.md`,
`cli/caprun/tests/live_acceptance_v1_10_cli.rs` (full), `scripts/compose-verify.sh`
(full, 212 lines), `scripts/check-invariants.sh` (Gate 5),
`crates/runtime-core/src/plan_node.rs`, `crates/runtime-core/src/executor_decision.rs`,
`cli/caprun/tests/live_acceptance_v1_8_composed.rs`.

**`51-04-SUMMARY.md` does not exist.** Per the prompt this is recorded, not failed — the
Task 3 human checkpoint is still open, and Plan 51-04's `<output>` requires the summary
only when Task 3 closes. Consistent, not a defect.

| Command | Exit | Result |
|---|---|---|
| `sha256sum` ×3 retained logs | 0 | see Hash verification |
| `git log --oneline -20`, `git status --porcelain`, `git rev-parse HEAD` | 0 | HEAD = `43fdb67` |
| `git diff --stat cb34b91..HEAD` | 0 | 7 files, all `.planning/` |
| `git show --stat 43eb822 / 43fdb67 / 171bdc0 / 54bca8e` | 0 | scopes confirmed |
| `git diff cb34b91..HEAD -- REQUIREMENTS.md WINDOWS.md` | 0 | reviewed in full |
| `git diff --stat 3e6e389..HEAD -- crates/ cli/` | 0 | **1 file: the test only** |
| `git show 3e6e389:…live_acceptance_v1_10_cli.rs \| sed -n '385,410p'` | 0 | pre-repair oracle |
| `grep -c` on both green logs for 7 claimed assertion strings | 0/1 | **0 hits in scoped log** |
| `grep -c` on both green logs for 10 environment facts | 1 | **0 hits, all facts, both logs** |
| `grep -nE "FAILED\|failures:\|error\[\|error:\|panicked\|timeout\|OPENAI"` both green logs | 1 | no real failures |
| `grep -n "test result:" 51-LIVE-FULL.log` | 0 | 77 lines, all `ok`, all `0 failed` |
| `grep -nE "^\s+Running (tests\|unittests)"` full log | 0 | 70 binaries enumerated |
| `git log --oneline -15 -- crates/brokerd/src/audit.rs` | 0 | newest = `442a056` (51-07) |

## Hash verification

All three match exactly. No discrepancy.

| Log | Claimed | Recomputed | Verdict |
|---|---|---|---|
| `51-LIVE-SCOPED.log` | `dc57e49b…6b98d` | `dc57e49b2d75ec0d040da69a780f13485507148b5efe19d0457640d41916b98d` | MATCH |
| `51-LIVE-FULL.log` | `4bcb275b…4ee3e` | `4bcb275b98dde637d7ac644a60227d33cc5ec47acf65e8898e2f1a4d4b34ee3e` | MATCH |
| `…FAIL-PRIOR-9b78bd4f.log` | `9b78bd4f…febbc` | `9b78bd4f974ade0b74db03c09b133e60e3395e514d08a5ba73bc624988afebbc` | MATCH |

## Scoped proof trace

**Command fidelity (check 2): confirmed exact.** The evidence command
(`51-LIVE-EVIDENCE.md:16`) is character-identical to Plan 51-04's `<action>` /
`<verification>` string, including `--features live-proof-fixtures,mock-egress-ca` and
`-- --test-threads=1`. No weakened flag, no dropped feature, no substituted host-native
command. `compose-verify.sh:203` captures the container's true `rc` **before** any pipe
and re-exits it (`:207-210`); the terminal `Composed Linux verification suite PASSED`
(`:212`) is only reachable on `rc == 0`. The `apt` step installs `pkg-config` only
(`libssl-dev` already newest) — package selection matches the CLAUDE.md-documented
recipe, no additions.

**Execution, not compilation (check 3): confirmed.** `51-LIVE-SCOPED.log:509-515`:

```text
running 4 tests
test linux::live_07_cli_multi_node_one_session_verify_chain ... ok
test linux::live_08_cli_mid_loop_i2_block_genuine_taint ... ok
test live_08_attribution_is_independent_of_exit_event_order ... ok
test live_acceptance_v1_10_cli_guard_present ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 60.94s
```

All four required tests present, zero failed, zero ignored, zero filtered. The two
`linux::`-prefixed tests are gated by
`#[cfg(all(target_os = "linux", feature = "mock-egress-ca", feature = "live-proof-fixtures"))]`
(`live_acceptance_v1_10_cli.rs:116-121`) — they cannot appear in the roster unless all
three conditions held, so this is affirmative proof of real Linux with both features on.
The 60.94s duration is consistent with two real `caprun run` subprocess trees;
compilation is separately accounted (`:423`, `:506`).

**Checks 4 and 5 — LIVE-07 / LIVE-08 facts.** Every claimed fact is a hard assertion that
must have held for the `ok` above, verified against the source at the executed revision:

| Claim | Assertion | Line |
|---|---|---|
| LIVE-07 exit 0 | `assert_eq!(status.code(), Some(0))` | `:405` |
| exactly one Session | `assert_eq!(session_ids.len(), 1)` | `:415` |
| git push succeeded | `count_events(…,"git_push_succeeded") == 1` | `:422` |
| GitHub PR succeeded | `count_events(…,"github_pr_succeeded") == 1` | `:426` |
| audit chain passes | `caprun audit` rc 0 **and** stdout contains `Chain verification: PASSED` | `:343-356` |
| LIVE-08 not success | `matches!(status.code(), Some(2\|3))` | `:484` |
| not policy_deny | `assert!(!stdout.contains("DENIED code=policy_deny"))` | `:489` |
| unique anchor = `github.pr`/`body` | filter `sink.0 == "github.pr" && arg == "body"` over **all** `sink_blocked` anchors, `len() == 1` | `:504-513` |
| genuine untrusted taint | `!taint.is_empty()` **and** `taint.iter().any(is_untrusted)` | `:515-516` |
| `read_event_id` selects `process.exec` exit | id-equality match, then `actor.starts_with("sink:process.exec:")` | `:52-61` |
| first provenance root | `provenance_chain.first() == Some(&process_event.id)` | `:62` |
| `git.commit` exit not creditable | exactly 2 exits, exactly 1 `sink:process.exec:`, exactly 1 `sink:git.commit:` | `:30-50` |
| no PR effect | `count_events(…,"github_pr_succeeded") == 0` | `:500` |
| audit chain passes | `assert_audit_passed` | `:499` |

`is_untrusted()` (`plan_node.rs:52-64`) returns `true` for `ExecRaw` and `false` for
`UserTrusted`/`LocalWorkspace`, so the taint assertion is non-vacuous.

**However (check 9, and the basis of MAJOR-01): none of these facts appears in the
retained scoped log.** Grep counts over `51-LIVE-SCOPED.log`: `Chain verification` = **0**,
`github_pr_succeeded` = **0**, `git_push_succeeded` = **0**, `process_exited` = **0**,
`github.pr` = **0**, `Session` = **0**. `cargo test` swallows a passing test's output, so
the log contains only names and `ok`. The facts are true and each was confirmed — but by
reading source, not the hashed artifact.

**Check 6 — adversarial oracle inspection.** Traced, not name-matched:

- **Row ordering** — defeated. `events_of_type` (`:325-341`) issues
  `SELECT payload FROM events WHERE …` with **no `ORDER BY`**, so row order is
  unspecified; but selection is by `event.id == anchor.read_event_id` (`:52-56`), never by
  index. `live_08_attribution_is_independent_of_exit_event_order` (`:67-108`) proves
  forward and reversed arrays yield the same id. Sound.
- **Duplicate anchors** — defeated. The filter runs across `sink_blocked` events
  flat-mapped over all anchors, then asserts `len() == 1` (`:509-513`). A second
  `github.pr`/`body` anchor fails the test.
- **Duplicate effects** — defeated. `github_pr_succeeded == 0` counts durable events, not
  stdout.
- **Actor-prefix ambiguity** — defeated. `sink:process.exec:` and `sink:git.commit:` are
  non-overlapping prefixes, and cardinality is pinned at exactly one each out of exactly
  two (`:30-50`). Neither can absorb the other.
- **Empty taint** — defeated by the conjunction at `:515-516`.
- **Causal-parent / provenance conflation** — defeated. The oracle never reads
  `event.parent_id`. The order-independence fixture deliberately gives the `git.commit`
  event `Some(process_id)` as its *causal parent* (`:84-92`) and still requires the answer
  to be the `process.exec` event — that fixture exists specifically to make conflation
  fail. This matches `51-ADVERSARIAL-TRACE.md` check 3, which independently confirmed
  `server.rs:941/957/973` keep `read_event_id` out of `parent_id`.
- **Event stapling** — *partially* defeated, and this is the honest residual limit. The
  oracle proves the durable anchor's provenance root is a real, correctly-typed
  `process_exited` event in the same Session's chain, and that the chain hashes verify. It
  does **not** independently recompute `anchor.literal_sha256` against the actual
  `process.exec` stdout, so "the body literal is byte-derived from that exit" rests on the
  executor/broker propagating provenance correctly — the component under test. That is a
  bounded self-attestation, mitigated by the audit-chain MAC and by 51-08's independent
  trace, not eliminated. Not raised as a finding: it is inherent to proving a taint
  runtime with its own audit DAG, and PLAN.md's §9 bar (unbroken taint edge in the DAG) is
  met.

**Was the oracle weakened to make it pass?** No — it was strengthened. Pre-repair
(`3e6e389`) it asserted `count_events(…,"process_exited") == 1` and selected
`process_events[0]` positionally; the real run emits two exits (`process.exec` +
`git.commit`), so it failed `left: 2, right: 1`. The repair replaced a false cardinality
premise and a positional selection with id-bound attribution plus explicit per-actor
cardinality. Strictly more constraining.

## Full-workspace proof trace

**Command fidelity: exact match** to Plan 51-04 Task 2, including `--no-fail-fast` and
`--features brokerd/mock-egress-ca`.

**Check 7 — confirmed.** 77 `test result:` lines, every one `ok` with
`0 failed; 0 ignored`. No `failures:` block, no `error[`/`error:`, no panic, no timeout,
no `FAILED` anywhere. `--no-fail-fast` means a later failing binary could not have been
masked by an earlier success, and none exists. 70 test binaries ran, ending with
`Composed Linux verification suite PASSED (Mailpit + mock GitHub).` The Phase 50
regressions (`coding_cli`, `stream_hold`, `stream_substrate`, `planner`) and
`linux::live_acceptance_v1_9_composed_success_chain` (`:1318-1324`) all passed, as
claimed.

**Check 8 — the coverage caveat the evidence omits.** Two suites took the documented
`OPENAI_API_KEY`-absent skip branch and passed vacuously:

- `live_acceptance_v1_4_composed_three_legs` — `finished in 0.00s`
  (`51-LIVE-FULL.log:1294-1302`); skip branch at `live_acceptance_v1_4_composed.rs:585-589`.
- `llm_planner_clean_allow_delivers` — `finished in 0.00s` (`:1326-1332`); skip branch at
  `llm_planner_live_accept.rs:206-210`.

`compose-verify.sh:190` forwards `OPENAI_API_KEY="${OPENAI_API_KEY:-}"` empty-tolerantly,
and `cargo test` hides a passing test's `eprintln!`, so the `SKIP …` lines are invisible
in the log — the 0.00s durations are the only tell.
`live_acceptance_v1_8_composed.rs:56-57` states outright that with the key absent the
sidecar path "is structurally-verified only and MUST NOT be claimed as a passed live
path." `51-LIVE-EVIDENCE.md:48` says all binaries "completed without failure" (true) but
nowhere discloses these legs. No compiler text was mistaken for failure; the `sandbox` /
`caprun` unused-import and dead-code warnings are harmless.

Separately: the full run executed only **2** of the 4 tests in
`live_acceptance_v1_10_cli` (`:1278-1284`), because `live-proof-fixtures` is not in the
full command's feature set. Correct by design — the scoped gate owns LIVE-07/08 — and the
evidence never claims otherwise, but it is also not stated.

## Evidence-to-log reconciliation

| `51-LIVE-EVIDENCE.md` | Status |
|---|---|
| `:16` scoped command | matches Plan 51-04 exactly |
| `:19` exit 0 via pipefail tee | inferred correctly — `PASSED` at log `:517` is unreachable on non-zero `rc` |
| `:21` scoped SHA-256 | recomputed, matches |
| `:22` both result excerpts | verbatim at log `:515` and `:517` |
| `:24` LIVE-07 four facts | **true but absent from the log** — asserted at test `:405-429`, zero grep hits in log |
| `:26` LIVE-08 six facts | **true but absent from the log** — asserted at test `:484-521`, zero grep hits in log |
| `:32-34` prior failed log, hash, exit 101, "not evidence for the green result" | exact; tail confirms `test result: FAILED` and `exited 101` |
| `:41` full command | matches Plan 51-04 exactly |
| `:47` full SHA-256 | recomputed, matches |
| `:48` all binaries green + terminal line | 77/77 ok; tail verbatim |
| `:49` Phase 50 + v1.9 legs passed | at log `:1318-1324`, `:1199-1219`, `:1425-1458` |
| `:50` invariants `All invariant gates PASSED`, Gate 5 skip | **no retained artifact**; Gate 5 skip text confirmed real at `check-invariants.sh:249` |
| `:5-9` host/OS/kernel/Docker/commit/timestamp | **zero occurrences in either log** — `uname`, `Ubuntu`, `6.17.0`, `aws`, `Docker version`, `29.7.2`, `cb34b91`, `2026-08-08`, `16:41`, `16:44` all grep to 0 in both |

No selective excerpt hides contradictory nearby output — the scoped log was read in full
and the full log's every result line checked; there is no adjacent failure cropped around
any quoted string.

What *is* independently established about the environment despite the missing metadata:
the run was on real Linux under Docker at revision `cb34b91`. Evidence: `landlock` /
`seccompiler` compiled and `linux::`-cfg tests executed; Debian trixie apt transcript and
three image pulls with digests (`:3-60`); compose sidecar orchestration on
`203.0.113.0/24`; and `git diff --stat cb34b91..HEAD` touching only `.planning/`. The
*specific* instance id, account, kernel string, Docker version, and UTC timestamps are
unbacked executor metadata.

**Check 10 — commits.** `43eb822` adds only `51-LIVE-EVIDENCE.md` + the two scoped logs.
`43fdb67` adds `51-LIVE-FULL.log` and edits only `51-LIVE-EVIDENCE.md`,
`51-VALIDATION.md`, `REQUIREMENTS.md`, `WINDOWS.md`. **Neither touches product code,
tests, or `compose-verify.sh`**, and neither modifies a log after hashing (each log is
added once, in one commit, never re-edited). The failed log is preserved unmodified under
a filename encoding its own hash and its failure status, and `51-LIVE-EVIDENCE.md:34`
explicitly labels it "not evidence for the green result above." No green-washing.

**Check 12 — post-proof mutation.** `git diff --stat cb34b91..HEAD` = 7 files, **all under
`.planning/`**. `git diff --stat 3e6e389..HEAD -- crates/ cli/` = **1 file,
`cli/caprun/tests/live_acceptance_v1_10_cli.rs` only**. The claim that the logs correspond
to `cb34b91`'s source is not contradicted by any later source or test mutation. The
working tree adds no source change (only `M .planning/STATE.md` and the untracked review
prompt).

## Validation, requirements, and windows reconciliation

- `nyquist_compliant: true` (`51-VALIDATION.md:5`) — **justified**. Every per-task row now
  cites an executed artifact (`scoped compose log dc57e49b…` / `full compose log
  4bcb275b…`) rather than the pre-execution `⚠️ compose pending` / `⬜ pending`
  placeholders it replaced. The row for "process_exited precedes Block; verify_chain true
  after Block" (`:48`) is honestly backed. `51-VALIDATION.md:24` was also corrected to the
  actually-executed scoped command.
- LIVE-07 / LIVE-08 — **complete in both representations**: `- [x]` checkboxes and
  `| LIVE-07 | Phase 51 | Complete |` / `| LIVE-08 | Phase 51 | Complete |` matrix rows.
  Requirement text was not softened; the diff flips `[ ]`→`[x]` and `Pending`→`Complete`
  with prose byte-identical.
- Windows 1–3 — all three moved `open`→`fixed` with real `resolved_at` timestamps,
  `open_count: 3`→`0`, `fixed_count: 0`→`3`, via the ledger tool (schema-consistent, JSON
  block and table agree). Windows 1 and 2 ("Docker not installed on executor host") are
  squarely fixed by an executed Docker compose run. Window 3 — see Q8.
- **No unrelated requirement or window was changed.** The `REQUIREMENTS.md` diff touches
  exactly the two LIVE lines and two matrix rows; the `WINDOWS.md` diff touches only the
  three Phase 51 entries and the counts.
- **ROADMAP and phase-completion state were NOT prematurely marked complete by the
  executor.** `ROADMAP.md:190` still `- [ ]`, `:327` still carries the "Phase 51 is NOT
  complete" block, and `STATE.md` is uncommitted in both commits — exactly as Plan 51-04
  required ("Preserve ROADMAP.md and STATE.md for the orchestrator").

## Teardown verification limits

The executor reports instance `i-0fb4d663c0097fdc5`, its key pair, security group, tagged
active volumes, and the local private key removed, with zero residual `Project=caprun`
resources.

**Independently verifiable from retained repository evidence: none of it.** No AWS CLI
transcript, no `describe-instances` output, no deletion receipt is retained anywhere in the
phase directory. The instance id itself appears only as executor-supplied prose at
`51-LIVE-EVIDENCE.md:5` and is absent from both logs.

**No AWS query was performed.** No read-only `describe-instances`, `describe-volumes`, or
tag filter was run; this reviewer holds no authorization for account `559846026666` and the
prompt forbids claiming external verification without actually performing it. Therefore
the entire teardown claim — existence, identity, and removal of every named resource —
**requires trust in external AWS state and is unverified by this review**. Cost and
residual-exposure risk from a missed resource cannot be excluded from repository evidence
alone. This is a limitation of scope, not a finding against the proof: teardown is not a
Plan 51-04 acceptance criterion.

## Adversarial questions and answers

1. **Could the scoped command exit zero while either LIVE proof silently did not execute?**
   **No.** Both `linux::`-prefixed tests are named in the roster with `... ok` (`:510-511`).
   Their `cfg(all(target_os="linux", feature="mock-egress-ca", feature="live-proof-fixtures"))`
   gate means a missing feature or non-Linux target would have produced a 2-test roster,
   not a silently-passing 4-test one. The prior failed run at the same gate (3 tests, one
   `FAILED`, rc 101) demonstrates the harness does surface failures.
2. **Could LIVE-08 pass while attributing the blocked body to `git.commit` rather than
   `process.exec`?** **No.** `attributed_process_exit` asserts the anchor-named event's
   actor `starts_with("sink:process.exec:")` (`:58-61`) after asserting exactly one exit of
   each actor kind. A `git.commit` attribution fails on the actor assertion; a fabricated
   third exit fails the `len() == 2` assertion.
3. **Could LIVE-08 pass with a PR effect despite the claimed block?** **No.**
   `count_events(…, "github_pr_succeeded") == 0` (`:500`) reads the durable audit DB, and
   the driver exit is pinned to `Some(2|3)` (`:484`). The sidecar deliberately never
   confirms the blocked PR node (`:471-472`), preserving the no-effect claim by
   construction.
4. **Could audit verification pass because the verifier or MAC contract was weakened after
   the reviewed fix?** **No — mechanically excluded.**
   `git diff --stat 3e6e389..HEAD -- crates/ cli/` returns exactly one file: the test.
   `crates/brokerd/src/audit.rs` has had no commit since `442a056` (51-07), which is the
   diff 51-08's independent reviewer traced and found `verify_chain`, `current_chain_head`,
   `compute_event_hash`, `verify_event_hash`, `compute_anchor_mac`, and `verify_anchor_mac`
   unchanged.
5. **Could the evidence markdown claim more coverage than the full log provides?**
   **Yes, and it partly does.** The "no v1.0–v1.9 regression" framing does not disclose
   that `live_acceptance_v1_4_composed_three_legs` and `llm_planner_clean_allow_delivers`
   self-skipped on the absent API key (both 0.00s), nor that only 2 of 4
   `live_acceptance_v1_10_cli` tests ran in the full pass. See MAJOR-02 / MINOR-03.
6. **Could a hash match while the excerpts mischaracterize the hashed file?**
   **Yes in principle, and this is the live risk here.** The two directly-quoted excerpts
   (`:22`) are verbatim. But `:24` and `:26` narrate in observational voice ("The passing
   test observed…", quoting `Chain verification: PASSED` as a literal) facts that occur
   **zero** times in the hashed file. They are true assertions of the test binary, not
   transcriptions of the artifact. See MAJOR-01.
7. **Were any status ledgers updated before both commands were known green?** **No.**
   `43eb822` (16:42:35Z) retains only scoped evidence and changes no status. Every ledger
   change — `REQUIREMENTS.md`, `WINDOWS.md`, `51-VALIDATION.md` — lands in `43fdb67`
   (16:46:51Z), after the full log (completion 16:44:49Z, ledger timestamps
   16:46:35–36Z). Ordering is correct and strictly gated.
8. **Is closing broken window 3 justified by the containerized full-workspace build, or
   does its original wording require a different check?** **Justified, with a caveat.**
   Window 3 records a *deferred cargo verification*, not a host-provisioning obligation;
   that verification has now run, with `pkg-config` installed and `libssl-dev` already
   present inside the container (`51-LIVE-SCOPED.log:72-93`) and the full workspace built
   and tested. The caveat: Plan 51-04's `<artifacts>` names "broken-window entries 1 and 2"
   only, while its own `<automated>` verify demands zero open Phase-51 entries — closing 3
   satisfies the executable gate and contradicts the prose inventory. The executor followed
   the stronger, executable requirement. Non-blocking; see MINOR-05.
9. **Does any unresolved finding from `51-ADVERSARIAL-TRACE.md` invalidate this evidence
   approval?** **No.** That review returned BLOCKER 0 / MAJOR 0. MINOR-01 (a comment
   overstating DB atomicity at `server.rs:1044-1049` while failures remain fail-closed
   under the broker mutex) and NIT-01 (stale "19 sites" comment at `audit.rs:1033-1037` vs
   45) are both comment-accuracy follow-ups that touch no enforcement path and cannot
   affect LIVE-07/08 outcomes. Its own verdict correctly disclaims constituting real-Linux
   proof.
10. **Is there any reason a cautious reviewer should require another real-Linux execution
    before approval?** **No, not for LIVE-07/08.** The scoped gate is sound, the oracle is
    strictly stronger than the one that failed, the source is provably unchanged since the
    proven revision, and the hashes match. Both MAJORs are evidence-*record* defects fixable
    by editing `51-LIVE-EVIDENCE.md` — no re-run needed. The one optional re-run worth
    considering is a full-workspace pass with `OPENAI_API_KEY` set, which would close
    MAJOR-02 substantively rather than by disclosure.

## Findings

### BLOCKER
Count: 0

### MAJOR
Count: 2
Unresolved MAJORs: 2

**MAJOR-01 — LIVE-07/LIVE-08 result facts are narrated as observed but appear nowhere in
the hashed log.**
Location: `51-LIVE-EVIDENCE.md:24` and `:26`; artifact `51-LIVE-SCOPED.log` (hash
`dc57e49b…`).
Consequence (falsifiable): `grep -c` over the hashed scoped log returns **0** for
`Chain verification`, `git_push_succeeded`, `github_pr_succeeded`, `process_exited`,
`github.pr`, and `Session`. A reviewer executing Plan 51-04 Task 3 step 4 verbatim —
"spot-check the evidence excerpts against the complete retained output" — cannot find the
quoted `Chain verification: PASSED` in the file whose hash they just verified, and would
reasonably conclude the record was fabricated. It was not: every one of these facts was
confirmed as a hard assertion at `live_acceptance_v1_10_cli.rs:405-429` and `:484-521`, in
source provably unchanged since `cb34b91`. The defect is provenance labelling, not truth.
Disposition: edit `51-LIVE-EVIDENCE.md` to attribute these facts to the named test
assertions with file:line citations (e.g. "asserted at
`cli/caprun/tests/live_acceptance_v1_10_cli.rs:422-429`, necessarily held for the logged
`ok`"), and state plainly that `cargo test` suppresses passing-test output so these strings
are absent from the log by construction. No re-execution required.

**MAJOR-02 — the full-workspace "no v1.0–v1.9 regression" claim does not disclose two
vacuously-passing LLM legs.**
Location: `51-LIVE-EVIDENCE.md:48-49`; log evidence at `51-LIVE-FULL.log:1294-1302` and
`:1326-1332`.
Consequence (falsifiable): `live_acceptance_v1_4_composed_three_legs` and
`llm_planner_clean_allow_delivers` each report `finished in 0.00s`, the signature of the
`OPENAI_API_KEY`-unset early-return at `live_acceptance_v1_4_composed.rs:585-589` and
`llm_planner_live_accept.rs:206-210`; `compose-verify.sh:190` forwards the key
empty-tolerantly and cargo hides the `SKIP` line on a pass.
`live_acceptance_v1_8_composed.rs:56-57` states the absent-key sidecar path "MUST NOT be
claimed as a passed live path." As written, the record's regression claim is broader than
the run supports, and a reader cannot detect the gap without timing forensics. This is the
precise overclaim pattern Phase 51 exists to eliminate.
Disposition: add a coverage caveat naming both legs and the reason. Optionally re-run the
full gate with a real `OPENAI_API_KEY` to convert the disclosure into coverage. Does not
affect LIVE-07/LIVE-08.

### MINOR
Count: 5

**MINOR-01 — host, kernel, Docker, commit, and timestamp facts have no retained
artifact.** `51-LIVE-EVIDENCE.md:5-9`, `:9`, `:44`. All ten strings (`uname`, `Ubuntu`,
`6.17.0`, `aws`, `Docker version`, `29.7.2`, `cb34b91`, `2026-08-08`, `16:41`, `16:44`)
grep to **0** in both green logs. Threat T-51-04-01 prescribes "Record
uname/Docker/commit/timestamp"; the record states them but captures none, so the spoofing
mitigation is only half-delivered. Consequence: an executor on a different host or revision
would produce an identical artifact set. Mitigating: real-Linux-under-Docker at `cb34b91`
is independently established as described above; only the specific identity is unbacked.
Disposition: label these lines as executor-supplied metadata, and in future runs tee
`uname -a`, `docker version`, `git rev-parse HEAD`, and `date -u` into the log head.

**MINOR-02 — `check-invariants.sh` output is not retained, and Gate 5 did not run.**
`51-LIVE-EVIDENCE.md:50`. Plan 51-04 Task 2 requires "green invariant Gates 1–6"; no
invariant transcript exists in the phase directory. The record does honestly disclose the
Gate 5 `cargo not found` skip, which is a real code path (`check-invariants.sh:249`) — so
Gate 5 (aws-lc-rs absence) was **not** verified on this run. Consequence: the aws-lc-rs
build-graph gate is unproven for the proof revision. Mitigating: the container built the
full workspace successfully. Disposition: retain the invariant output; run it where `cargo`
is on PATH.

**MINOR-03 — the full-workspace run covered only 2 of 4 `live_acceptance_v1_10_cli` tests,
undisclosed.** `51-LIVE-FULL.log:1278-1284`. The `linux::` LIVE-07/08 tests are absent
because the full command omits `live-proof-fixtures`. Correct by design and never
misclaimed, but a reader may assume the full pass re-proved LIVE-07/08. Disposition: one
clarifying sentence.

**MINOR-04 — the working tree carries a stale, self-contradictory uncommitted
`STATE.md`.** The unstaged diff *degrades* accuracy: it replaces HEAD's correct "51-04
STOPPED at Task 1 … failed" position with "Plan 1 of 8 / Status: Executing Phase 51 / Last
activity: 2026-08-05 — Phase 51 execution started", stamps `last_updated: 2026-08-05`
(three days *before* the proof), and leaves the now-false line "Phase 51 proof gate OPEN —
LIVE-07/LIVE-08 Pending, windows 1/2/3 open, no 51-LIVE-EVIDENCE.md". Consequence: whoever
commits this tree records a state contradicting `REQUIREMENTS.md` and `WINDOWS.md`. Not
caused by `43eb822`/`43fdb67` — both correctly left `STATE.md` alone per plan. Disposition:
discard or correct before the orchestrator's post-approval state update.

**MINOR-05 — window 3 closed beyond the plan's named artifact list, and all three `reason`
fields left empty.** `WINDOWS.md` table and JSON. Plan 51-04 `<artifacts>` names entries 1
and 2; its `<automated>` verify requires zero open Phase-51 entries. The executable gate
wins, so the action is defensible (see Q8), but the empty `reason: ""` on three `fixed`
entries loses the justification that would let a future reader confirm *why* each closed.
Disposition: populate `reason` with the evidence pointer; reconcile the plan's prose with
its verify command.

### NIT
Count: 3

**NIT-01** — `ROADMAP.md:190` (`- [ ]`) and `:327-331` ("Phase 51 is NOT complete… no
`51-LIVE-EVIDENCE.md` exists… Task 2 never ran") now contradict `REQUIREMENTS.md`. This is
*correct* executor behavior — Plan 51-04 mandates preserving ROADMAP for the orchestrator —
but the contradiction must be cleared once the checkpoint closes.

**NIT-02** — `51-VALIDATION.md:5` frontmatter `status: draft`→`complete` asserts phase
completion while the blocking human checkpoint (Task 3) is open. The plan explicitly
authorizes updating `status`, so this is in scope; flagged only because "complete" precedes
the gate that defines completion.

**NIT-03** — `51-04-SUMMARY.md` absent. Expected and correct while Task 3 is open; recorded
per the prompt, not counted against the record.

## Final verdict
NEEDS MORE EVIDENCE

Hashes match on all three logs; both gates genuinely executed on real Linux under Docker
and exited 0; the scoped run shows all four required tests passing with zero failures in
60.94s; the LIVE-08 oracle survives every attack in check 6 and is strictly stronger than
the one it replaced; the full run is 77/77 green with no masked failure; the two commits
touched only `.planning/`; no verifier or MAC was weakened after the 51-08 trace; ledgers
were updated strictly after both greens; and ROADMAP/STATE were correctly left to the
orchestrator. **The underlying proof is sound and no further real-Linux execution is
recommended for LIVE-07/LIVE-08.**

Approval is withheld solely because the prompt's bar requires **unresolved MAJOR 0** and two
remain, both confined to `51-LIVE-EVIDENCE.md`: results narrated as log-observed that are
absent from the hashed log (MAJOR-01), and an undisclosed skipped-LLM-leg coverage gap
behind the "no v1.0–v1.9 regression" claim (MAJOR-02). Both are correctable by editing the
evidence record — attributing the LIVE facts to their test assertions with file:line
citations and naming the two vacuous legs. Once those edits land, this record clears the
approval bar, and the remaining MINORs and NITs can be handled as follow-ups. The teardown
claim remains entirely unverified and should not be represented as independently confirmed.
