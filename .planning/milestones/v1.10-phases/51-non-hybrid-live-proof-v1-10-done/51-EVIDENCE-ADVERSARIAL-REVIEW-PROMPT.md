# Phase 51 Real-Linux Evidence — Independent Adversarial Review Prompt

You are a fresh, independent adversarial reviewer. Perform a **read-only** review of Phase 51's retained real-Linux proof before the human approves Plan 51-04. Do not edit, generate, delete, stage, or commit any file. Do not assume the executor's evidence record, summaries, status changes, or reported hashes are correct.

## Repository and review target

Repository root:

```text
/home/ben/Workspace/caprun
```

Review the live checked-out tree. The proof claims its executed source revision was:

```text
cb34b9124916164397697a948c0c4804db221c82
```

The proof was run on Ubuntu 24.04, Linux kernel `6.17.0-1019-aws`, Docker `29.7.2`, and Docker Compose `v5.4.0`.

## Required files

Open and inspect all of these files:

```text
.planning/phases/51-non-hybrid-live-proof-v1-10-done/51-04-PLAN.md
.planning/phases/51-non-hybrid-live-proof-v1-10-done/51-04-SUMMARY.md
.planning/phases/51-non-hybrid-live-proof-v1-10-done/51-09-PLAN.md
.planning/phases/51-non-hybrid-live-proof-v1-10-done/51-09-SUMMARY.md
.planning/phases/51-non-hybrid-live-proof-v1-10-done/51-LIVE-EVIDENCE.md
.planning/phases/51-non-hybrid-live-proof-v1-10-done/51-LIVE-SCOPED.log
.planning/phases/51-non-hybrid-live-proof-v1-10-done/51-LIVE-FULL.log
.planning/phases/51-non-hybrid-live-proof-v1-10-done/51-LIVE-SCOPED-FAIL-PRIOR-9b78bd4f.log
.planning/phases/51-non-hybrid-live-proof-v1-10-done/51-VALIDATION.md
.planning/REQUIREMENTS.md
.planning/WINDOWS.md
cli/caprun/tests/live_acceptance_v1_10_cli.rs
scripts/compose-verify.sh
```

If `51-04-SUMMARY.md` does not yet exist because the human checkpoint is still open, record that fact; do not treat it as an automatic failure. Use commits `43eb822` and `43fdb67` to inspect the executor's Task 1 and Task 2 changes.

## Mandatory mechanical checks

Run read-only commands sufficient to establish all of the following. Include the commands and exit codes in your report.

1. Recompute SHA-256 hashes for the three retained logs. The claimed hashes are:

   - Green scoped log: `dc57e49b2d75ec0d040da69a780f13485507148b5efe19d0457640d41916b98d`
   - Green full log: `4bcb275b98dde637d7ac644a60227d33cc5ec47acf65e8898e2f1a4d4b34ee3e`
   - Preserved prior failed scoped log: `9b78bd4f974ade0b74db03c09b133e60e3395e514d08a5ba73bc624988afebbc`

2. Confirm the evidence record names the exact commands required by Plan 51-04, with no weakened flags, omitted features, altered package selection, or substituted host-native command.

3. Confirm the green scoped log contains actual execution—not compilation alone—and reports all four scoped tests passing with zero failures:

   - LIVE-07
   - LIVE-08
   - the order-independence regression
   - the guard test

4. Confirm LIVE-07's logged assertions support every claimed fact: exactly one Session, successful git push, successful GitHub PR effect, and passing audit-chain verification.

5. Confirm LIVE-08's logged/tested assertions support every claimed fact:

   - the unique durable blocked anchor is exactly `github.pr` / `body`;
   - it carries genuine untrusted taint;
   - its `read_event_id` selects the intended `sink:process.exec:*` `process_exited` event;
   - that event ID is the first provenance root;
   - the separate `sink:git.commit:*` exit event cannot be accidentally credited;
   - no `github_pr_succeeded` effect exists;
   - the audit chain passes.

6. Adversarially inspect `live_acceptance_v1_10_cli.rs`. Determine whether the repaired oracle could pass through row ordering, event stapling, actor-prefix ambiguity, duplicate anchors, duplicate effects, an empty taint set, or causal-parent/provenance conflation. Do not merely repeat test names; trace the actual queries and assertions.

7. Confirm the full log shows the exact full-workspace command actually executed to completion, with all suites green, no hidden failing test after a successful subcommand, and final `Composed Linux verification suite PASSED` output.

8. Search both green logs for `FAILED`, `error:`, panic output, ignored failures, timeouts, skipped suites, and environment-gated skips. Distinguish harmless compiler text from real failures. In particular, identify any v1.4/v1.8 composed legs that skipped because `OPENAI_API_KEY` was absent and evaluate whether the evidence record describes that coverage honestly.

9. Compare `51-LIVE-EVIDENCE.md` line by line against the raw logs. Every environment fact, timestamp, revision, command, exit status, test result, and coverage caveat must either be directly evidenced or clearly labelled as metadata supplied by the executor. Flag any selective excerpt that hides contradictory nearby output.

10. Inspect commits `43eb822` and `43fdb67`. Confirm they contain only evidence/status changes authorized by Plan 51-04 and did not alter product code, tests, proof commands, or the retained logs after hashing. Confirm the failed log remains clearly identified as failure evidence rather than overwritten or presented as green.

11. Check `51-VALIDATION.md`, `.planning/REQUIREMENTS.md`, and `.planning/WINDOWS.md` against the green logs and Plan 51-04 acceptance criteria. Verify:

    - `nyquist_compliant: true` is justified;
    - LIVE-07 and LIVE-08 are complete in both checkbox and traceability representations;
    - Phase 51 windows 1–3 are actually fixed for evidence-backed reasons;
    - no unrelated requirement/window was changed;
    - ROADMAP and phase-completion state were not prematurely marked complete by the executor.

12. Inspect git history and the working tree for post-proof changes that could invalidate the claim that the logs correspond to revision `cb34b91`. Distinguish later documentation/status commits from any later source or test mutation.

13. Evaluate teardown evidence critically. The executor reported that instance `i-0fb4d663c0097fdc5`, its key pair, security group, tagged active volumes, and local private key were removed, with zero residual `Project=caprun` resources. State which portions are independently verifiable from retained repository evidence and which require trust in external AWS state. Do not claim external teardown verification unless you actually query the authorized AWS account read-only.

## Adversarial questions

Answer each explicitly:

1. Could the scoped command exit zero while either LIVE proof silently did not execute?
2. Could LIVE-08 pass while attributing the blocked body to the `git.commit` event rather than `process.exec`?
3. Could LIVE-08 pass with a PR effect despite the claimed block?
4. Could audit verification pass because the verifier or MAC contract was weakened after the reviewed fix?
5. Could the evidence markdown claim more coverage than the full log actually provides?
6. Could a hash match while the evidence excerpts mischaracterize the hashed file?
7. Were any status ledgers updated before both commands were known green?
8. Is closing broken window 3 justified by the containerized full-workspace build, or does its original wording require a different check?
9. Does any unresolved finding from `51-ADVERSARIAL-TRACE.md` invalidate this evidence approval?
10. Is there any reason a cautious reviewer should require another real-Linux execution before approval?

## Required report format

Return a report with these exact sections:

```markdown
## Reviewer identity and independence

## Files opened and commands run

## Hash verification

## Scoped proof trace

## Full-workspace proof trace

## Evidence-to-log reconciliation

## Validation, requirements, and windows reconciliation

## Teardown verification limits

## Adversarial questions and answers

## Findings

### BLOCKER
Count: N

### MAJOR
Count: N
Unresolved MAJORs: N

### MINOR
Count: N

### NIT
Count: N

## Final verdict
APPROVE | REJECT | NEEDS MORE EVIDENCE
```

Every finding must include severity, exact file/line or log excerpt location, a falsifiable consequence, and a recommended disposition. Approval is allowed only with **BLOCKER 0**, **unresolved MAJOR 0**, matching hashes, green scoped/full execution, and no material evidence/status overstatement. If any required file is missing, any hash differs, a claimed test did not execute, a status is unsupported, or external facts are presented as independently verified when they are not, do not approve.
