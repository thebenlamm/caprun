# Phase 44: `git.push` — Broker-Performed Destination-Pinned Egress — Research

**Researched:** 2026-07-18
**Domain:** git smart-HTTP push protocol glue in the Rust TCB; broker-mediated destination-pinned egress; confined-child pack generation; confirm-release audit discipline
**Confidence:** HIGH on live-code anchors (every `file:line` re-verified this session against HEAD `89cab31`); HIGH on the pinned realization; MEDIUM only on pkt-line/pack-gen glue effort sizing (the research's one standing MEDIUM).

> **This is a WIRING-GAP re-verification, not a mechanism re-decision.** The FORK is DECIDED
> (Candidate (b), broker-performed smart-HTTP transfer, `.planning/research/GIT-PUSH-EGRESS.md`
> + `DESIGN-v1.9-egress-policy.md §1`). Every DESIGN decision is treated as locked. My job is
> (1) confirm the anchors the plan will build on still exist as cited, and (2) surface the
> wiring the DESIGN left open — §6 is the highest-value output.

---

## Summary

The `git.push` network leg rides the **already-shipped Pattern-A egress locus** in
`crates/brokerd/src/sinks/http_request.rs` (reqwest 0.13.4 + rustls-ring + webpki-roots + SSRF
resolve-and-pin, live-proven on Linux since Phase 40). The child stays net-denied under the
**unchanged** `exec_child_filter` (`sandbox/src/seccomp.rs:147`, denies `AF_INET`/`AF_INET6`,
permits `AF_UNIX`) — identical to `git.commit`. The executor registration mirrors exactly how
Phase 43 added `http.request.write`; the confirm-release wiring mirrors the P33/P34/§9
precheck-before-burn discipline already implemented for `process.exec`/`github.pr`/
`http.request.write` in `confirmation.rs`. **Zero new crates** — the only new code is auditable
Rust protocol glue.

**But the shipped egress primitives do not fit a two-request-one-frozen-IP + binary-pack flow
verbatim.** Two concrete blockers the DESIGN names but the code does not yet provide: (WG-1) the
"resolve once, reuse the client across both requests" primitive does not exist — `resolve_and_pin`
resolves-and-discards-the-addr internally, and `build_pinned_client`/`vet_resolved` are
module-private; (WG-2) `run_launcher` captures combined stdout+stderr as a **lossy UTF-8 String
with no stdin** — a binary packfile cannot survive it, so pack generation needs a new
binary-capable confined-spawn variant. Plus a set of smaller-but-real gaps (§6): the git.push
dispatch model (auto-Allowed vs github.pr-style always-gated) is unpinned; an anti-TOCTOU
**new-oid freeze** must be threaded into the pending-confirmation at Block time; the `effect.rs`
`GitPush` ontology uses `branch` where the sink uses `refspec`; the mock harness has no
`git-receive-pack` endpoint.

**Primary recommendation:** Pin the realization as **broker-builds-the-command-list (pure Rust)
+ confined-child `git pack-objects --revs --stdout --thin` for the PACK (binary capture)**, NOT
`git send-pack --stateless-rpc` (MEDIUM-confidence, needs bidirectional binary piping — strictly
more surface). Structure Phase 44 as **4 plans / 3 waves** mirroring Phase 43: executor-TCB
registration ∥ broker egress module → dispatch+confirm-release → differential acceptance + mock
+ HYG-01.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Destination pin (IP:port frozen) | Broker application layer (reqwest `.resolve`) | — | Only locus that SEES the destination IP; seccomp provably cannot pin a `connect()` sockaddr (BLOCKER-1, locked) |
| TLS termination + HTTP mover | Broker (reqwest/rustls-ring) | — | Child is net-denied; TLS+socket live broker-side |
| Pack generation (read `.git`, compute PACK) | Broker-spawned **confined child** (Pattern B) | — | Reuses git.commit's Landlock+seccomp+config-neutralization; keeps the git-config RCE surface contained exactly as git.commit does |
| pkt-line framing + report-status parse | Broker (pure Rust glue) | — | Net-new auditable Rust; no external binary |
| Sink registration + I2 sensitivity + effect class | Executor TCB (`crates/executor`) | `runtime-core` (PRODUCTION_SINKS, effect ontology) | HARDCODED sensitivity; policy narrows separately |
| Structural `--force`/`:delete`/`+refspec` denial | Broker command-list construction + a broker arg-validator | Executor Step-0 (name-set only) | Value-level refusal (mirrors `validate_write_method`), unreachable even via confirm |
| Confirm-release + payload-provenance surface | Broker `confirmation.rs` | — | P33/P34 terminal-event-before-terminal-state; renders commit-range + taint summary |

---

## §1. Verified Live-Code Anchor Table

All paths relative to repo root; line numbers verified at HEAD `89cab31` (Phase 43 landed).

| # | Anchor | file:line | Reusable verbatim? | What git.push needs |
|---|--------|-----------|--------------------|---------------------|
| A1 | SSRF resolve-and-pin core | `http_request.rs:492 resolve_and_pin` | **NO — re-resolves each call** | Returns a `Client` after `vet_resolved`+`build_pinned_client`; **discards the `SocketAddr`**. Cannot freeze one IP across two requests. See WG-1. |
| A2 | Vetted-addr → pinned client | `http_request.rs:394 build_pinned_client(host, pinned)` | **YES (logic)** — but **private `fn`** | The DESIGN-named reusable primitive (`§1.5`). Takes an already-vetted `SocketAddr`, sets `redirect(Policy::none())` (`:396`), `.resolve(host,pinned)` (`:398`), ring TLS. **Visibility must widen to `pub(crate)`.** |
| A3 | SSRF classifier | `http_request.rs:236 ssrf_check` (`pub fn`), `374 vet_resolved` (private) | `ssrf_check` YES; `vet_resolved` needs `pub(crate)` | Never re-implement; invoke both. `vet_resolved` (`:374`) fail-closes on any denied IP in a mixed answer. |
| A4 | URL validation | `http_request.rs:196 validate_url` | YES | Rejects `userinfo@`, non-https, explicit ports, IP-encoding tricks. Apply to the push remote URL. |
| A5 | Redirect refusal | `http_request.rs:396 .redirect(Policy::none())` inside A2 | YES (inherited by any client from A2) | Governs BOTH requests automatically if both ride ONE A2-built client (§1.5). |
| A6 | Body byte cap | `http_request.rs:89 check_body_cap`, `80 MAX_RESPONSE_BODY_BYTES` | YES | Cap the report-status response read. |
| A7 | Existing authenticated POST | `http_request.rs:567 invoke_pinned_post` | **NO — do NOT reuse** | Calls `resolve_and_pin` internally (`:592`) → re-resolves → reopens the DNS-rebind window §1.5 exists to close. DESIGN MINOR-3 forbids reuse. |
| A8 | Phase-43 WRITE egress | `http_request.rs:658 invoke_http_write` + `183 validate_write_method` | Pattern only | Distinct `WRITE_HOST_ALLOWLIST` (`:119`, empty in release; mock-only host under feature). git.push likely needs its OWN receive-pack host-allowlist (WG-9). `validate_write_method` (`:183`) is the **exact pattern** for a `validate_git_refspec` value-gate. |
| A9 | Phase-43 write sink module | `http_write.rs` — `prepare_http_write:123`, `invoke_http_write_from_resolved:310`, `invoke_http_write_sink:270`, opaque audit `append_write_outcome:164`, broker-env cred `write_bearer:80` (`WRITE_TOKEN_ENV="CAPRUN_HTTP_WRITE_TOKEN":62`) | **Template to clone** | Reusable: two-phase opaque audit shape, broker-env-only credential, `prepare_*`/`_from_resolved` split. Distinct for git.push: a `CAPRUN_GIT_PUSH_TOKEN` env var, the two-request transfer, binary pack gen. |
| A10 | github.pr sink | `github_pr.rs` — `prepare_github_pr`, `invoke_github_pr_from_resolved`, grant/CAS | **Mirror grant-gate, OMIT CAS** | github.pr requires a live auth-grant even on an Allowed decision (`server.rs:1355`). Decide whether git.push mirrors this (WG-3). CAS duplicate-suppression is github.pr-specific; git.push has no dedup requirement per DESIGN. |
| A11 | Pattern-B confined launcher | `process_exec.rs:402 run_launcher`, `385 SAFE_EXEC_PATH`, `422 env_clear()`, `373 resolve_launcher_path` | **NO for binary — YES for text** | Returns `(ExitStatus, String)` via `String::from_utf8_lossy(&combined)` (`:490`), stdout+stderr **merged**, **no stdin**. Fine for `git rev-parse` (text); corrupts a binary PACK. See WG-2. |
| A12 | seccomp net-deny child filter | `seccomp.rs:147 exec_child_filter` (AF_INET/AF_INET6 `→EPERM` `:163-188`, `AF_UNIX` permitted, no execve-deny) | **YES verbatim** | The pack-gen child's filter — unchanged from git.commit. NO relaxation. |
| A13 | Confined launcher self-confine | `caprun-exec-launcher/src/main.rs:71 exec_child_ruleset(workspace_root)` + `exec_child_filter` | YES | Landlocks the child to the workspace root → pack-gen child can read `.git`. |
| A14 | git.commit sink (Pattern B) | `git_commit.rs:91 invoke_git_commit`, config/hook neutralization `-c core.hooksPath=/dev/null`+`GIT_CONFIG_NOSYSTEM=1`+`GIT_CONFIG_GLOBAL=/dev/null` (`:119,138-141`), exit-code gating (`:177`), cwd=workspace via `run_launcher(...Some(cwd)...)` (`:150`) | **YES — clone the confinement recipe** | Pack-gen child reuses the SAME neutralization + workspace cwd. Exit-code gating is the model (non-zero pack-objects = sink failure). |
| A15 | Executor sink registration | `sink_schema.rs:40 KNOWN_SINKS` (http.request.write row `:138 allowed/required=["url","body","method"]`); `sink_sensitivity.rs:40 sink_effect_class` (`http.request.write=>CommitIrreversible :85`, `_ =>CommitIrreversible :97`), routing table `:228`, content table `:250`, `expected_role :294` | **Exact clone pattern** | Add `git.push` rows: schema `{remote,refspec}`, effect_class `CommitIrreversible` (explicit + `_ =>` default backstop), routing-sensitive `{remote,refspec}`, content-sensitive none, `expected_role None`. |
| A16 | I0/lifecycle deny | `executor/src/lib.rs:56 submit_plan_node`, Draft-deny fires only on `CommitIrreversible` (`:266-273`), non-live deny (`:292-296`) | YES (automatic) | Because git.push is `CommitIrreversible`, a draft/untrusted-seeded session cannot Allow it — I0 gate fires for free (like http.request.write). |
| A17 | PRODUCTION_SINKS | `runtime-core/src/policy.rs:34` (git.push **ABSENT**; test `:555` asserts NOT permitted) | **Must ADD** | Add `"git.push"` so `broker_default()` permits it (else PolicyDeny). Update the `:555` negative test. See WG-5. |
| A18 | Effect ontology | `runtime-core/src/effect.rs:27 IrreversibleEffect::GitPush { remote, branch }` | **Field mismatch** | Uses `branch`; sink args + DESIGN use `refspec`. Reconcile. See WG-4. |
| A19 | Confirm entry-guard | `confirmation.rs:845-847` allow-list (`file.create\|email.send\|file.write\|process.exec\|github.pr\|http.request.write`) | **Must EXTEND** | Add `"git.push"` (a confirm-releasable sink absent here is denied at the guard — REQUIRED, §1.7). |
| A20 | Confirm precheck-before-burn | `confirmation.rs:857 Step 4.8` (process.exec), `:877 Step 4.8b` (github.pr grant+precheck), `:933 Step 4.8c` (http.request.write) | **Add Step 4.8d** | `prepare_git_push` precheck HERE, before `confirm_granted` append (`:952-970`) + CAS burn (`:984`). Fail-closed-RECOVERABLE, row stays Pending. |
| A21 | Confirm Step-7 dispatch | `confirmation.rs:993 match pc.sink.0` (arms `:994 file.create`, `:1094 process.exec`, `:1136 github.pr`, `:1209 http.request.write`) | **Add git.push arm** | Async arm (awaits) folding EVERY pre-transfer/transport failure into a terminal `git_push_failed` FIRST — never a dangling `confirm_granted` (P33/P34). No mint (keeps Gate-3 mint-site list byte-identical). |
| A22 | server.rs Allowed-dispatch | `server.rs:1355` github.pr Allowed arm (grant-gated, no auto-POST); `:985 file.create`, `:1014 file.write`, `:1038 email.send` | **Add git.push arm (model TBD)** | If git.push mirrors github.pr, an Allowed decision is grant-gated, not auto-pushed. See WG-3. |
| A23 | render confirm block | `confirmation.rs:570 render_block_display`, taint labels `:523` | **Extend** | Must add commit-range + per-file taint-provenance summary for git.push (§1.6 / GIT-03). No shipped per-file summary exists. WG-8. |
| A24 | Gate 5 supply-chain | `check-invariants.sh:211-233` (workspace-scoped `cargo tree --workspace -i aws-lc-rs`/`openssl-sys`) | YES — re-runs | Zero new crates ⇒ stays green; re-run CONFIRMS no transport dep crept in (HYG-01). |
| A25 | Gate 4b + compose harness | `check-invariants.sh:180-189` (mock-egress-ca never default); `compose-verify.sh` mock GitHub sidecar `scripts/mock-github/server.py` | **Extend mock** | Mock answers only `POST /repos/*/pulls`; needs a `git-receive-pack` endpoint (WG-9). |
| A26 | llm-planner coupling | `llm-planner/src/lib.rs:599-605` asserts `git.push => UnknownSink` | **Update test** | Registering git.push flips this (if the planner learns it) — easy-to-miss coupling. WG-5. |

---

## §2. Pack-Generation Recommendation (with concrete git plumbing)

**PIN: broker-builds-the-command-list (pure Rust) + confined-child `git pack-objects` for the
binary PACK.** Reject `git send-pack --stateless-rpc` as the primary (research MEDIUM confidence;
needs bidirectional binary stdin/stdout piping — strictly more surface than pack-objects, whose
stdin is a tiny text rev-list and stdout is the pack). This matches DESIGN §1.1's stated
PREFERENCE (broker-generates-the-body-directly to avoid the send-pack invocation surface). Keep
send-pack as the §1.9-backstopped documented alternative only.

**Concrete sequence (all network on the ONE frozen-IP client from WG-1):**

1. **`GET {remote}/info/refs?service=git-receive-pack`** through the frozen-IP client.
   Parse the pkt-line advertisement: first pkt is `# service=git-receive-pack\n` then a
   flush; then ref lines. The **first** ref line is `<oid> SP <refname> NUL <capabilities>`
   (`report-status`, `report-status-v2`, `delete-refs`, `side-band-64k`, `agent=…`);
   subsequent lines are `<oid> SP <refname>`; terminated by a flush-pkt `0000`.
   → capture **old-oid** = the advertised oid of the target refname (or the special
   `0{40}`/`0{64}` zero-oid **create** case if the ref is not advertised).
2. **Resolve new-oid:** confined child `git rev-parse --verify <local-ref>^{commit}` — TEXT
   output, `run_launcher` (A11) fits verbatim. (Or the broker reads `.git`; rev-parse via the
   confined child is the least-surface, config-neutralized path.)
3. **Build the command-list (pure Rust):**
   `pkt_line("<old-oid> <new-oid> <refname>\0 report-status side-band-64k agent=caprun")`
   (capabilities on the FIRST — here only — command only), then `flush_pkt()` (`0000`).
   **Structural denial lives here (§1.3):** refuse to construct a line where `new-oid` is the
   zero-oid (**delete**); never emit a `+`-prefixed refname or any force capability; the
   `refspec` arg is validated by a broker `validate_git_refspec` (mirror `validate_write_method`
   A8) that rejects a leading `+`, an empty `<src>` (`:dst` deletion), and `--force*`-shaped
   tokens BEFORE construction.
4. **Generate the PACK:** confined child
   `git pack-objects --revs --stdout --thin --delta-base-offset` with **stdin** =
   `"<new-oid>\n^<old-oid>\n"` (omit the `^<old-oid>` line for a create). → **binary** packfile
   on stdout. Requires the WG-2 binary-capable launcher variant. `--thin` because receive-pack
   advertises thin-pack; the remote completes the thin pack from its objects.
5. **`POST {remote}/git-receive-pack`** through the SAME frozen-IP client:
   `Content-Type: application/x-git-receive-pack-request`, body = command-list pkt-lines ++ PACK
   bytes, `Authorization` from broker env (WG-3 credential). Parse the
   `application/x-git-receive-pack-result` response's `report-status`: `unpack ok\n` or
   `unpack <err>`, then per-ref `ok <refname>` / `ng <refname> <reason>`.

**Why a confined child (not broker-direct git exec) for pack-objects:** the workspace `.git` is
untrusted-worker-writable. Running `git pack-objects` unconfined in the broker with the repo
would read repo-local `.git/config` — a documented git-config RCE surface. Spawning it as the
Pattern-B confined child (net-denied, Landlocked to workspace, `env_clear`, git.commit's
`-c core.hooksPath=/dev/null`+`GIT_CONFIG_NOSYSTEM=1`+`GIT_CONFIG_GLOBAL=/dev/null`
neutralization) contains that surface exactly as git.commit already does (A14). The price is the
WG-2 binary-capture gap.

---

## §3. pkt-line / report-status Glue Spec + Honest Effort

**No shipped helper exists — this is net-new Rust glue.** (`grep` confirms zero pkt-line code in
`crates/`.) Minimal surface:

```rust
// ENCODE
fn pkt_line(payload: &[u8]) -> Vec<u8>   // 4-hex big-endian length (payload.len()+4) ++ payload
fn flush_pkt() -> &'static [u8]          // b"0000"
// DECODE
enum Pkt { Data(Vec<u8>), Flush }
fn read_pkt(buf: &mut &[u8]) -> Result<Option<Pkt>>  // read 4 hex, len==0 => Flush, else len-4 bytes
// ADVERTISEMENT parse: skip "# service=..." pkt + flush; first ref line splits on NUL for caps;
//   collect (refname -> oid); require the target ref's old-oid (or zero-oid create).
// REPORT-STATUS parse: unpack line + per-ref ok/ng; side-band-64k demux (band 1 = data) if negotiated.
```

**Honest size:** ~180–280 LoC of pure, host-portable Rust (encode/decode + two parsers), plus
the two-request driver (~80 LoC, Linux-gated socket legs) and the pack-gen child glue (~60 LoC).
This is the research's single MEDIUM. It is **fully unit-testable on macOS** (pkt framing, adv
parse, report-status parse, refspec-denial, command-list construction are all pure) — only the
socket + real `git pack-objects` legs are Linux-gated, matching the shipped `#[cfg(target_os =
"linux")]` split. Recommend a dedicated plan (44-02) for it. Side-band-64k demux can be
**avoided** by NOT advertising `side-band-64k` in the command-list caps (simplest correct
subset — the remote then writes report-status on the main band); recommend the planner start
without side-band and add it only if a target remote requires it.

---

## §4. Confirm-Payload Freeze (anti-TOCTOU, §1.6 / GIT-03)

**The DESIGN states the principle; the code needs a frozen-oid thread.** Concretely:

- At **Block time** (when a tainted `remote`/`refspec` Blocks and a pending-confirmation is
  inserted, `confirmation.rs:336 insert_pending_confirmation`), the **new-oid must be resolved
  and FROZEN into the pending confirmation's snapshot** (alongside the resolved args), and shown
  in the confirm prompt. The pending-confirmation already MACs its snapshot
  (`build_pending_confirmation_mac:252`) — the frozen new-oid rides that integrity boundary.
- At **confirm/dispatch time** (post-burn, Step-7 arm A21), the pack-gen child re-runs
  `git rev-parse` and the arm **refuses (terminal `git_push_failed`) if the live oid ≠ the
  frozen oid** — closing the window where a compromised worker advances the ref between Block and
  confirm. The human confirmed oid A; only oid A may be packed/pushed.
- **What the human sees at confirm (§1.6):** the frozen commit range (`<old-oid>..<new-oid>` /
  branch) **plus** a per-file provenance summary flagging any file in that range whose content
  derives from untrusted taint — computed from the value-store provenance the executor already
  tracks (WG-8; new rendering in `render_block_display` A23). Accepted residual (§1.6, carries
  v1.8 §11): this SURFACES provenance for human judgment; it does not itself Block
  untainted-but-sensitive content.

---

## §5. `--force` / `:delete` / `+refspec` Structural-Denial Locus

**Two defense-in-depth layers, both value-level (the executor Step-0 schema gate checks only the
arg NAME set — `sink_schema.rs:197 validate_schema` — so it cannot see a force refspec value):**

1. **Broker arg-validator `validate_git_refspec(refspec)`** — mirror `validate_write_method`
   (A8, `http_request.rs:183`): reject a leading `+` (force), an empty `<src>` in `<src>:<dst>`
   (`:dst` deletion), `--force`/`--force-with-lease`-shaped tokens. Called in BOTH
   `prepare_git_push` (Step 4.8d precheck) AND the transfer path — so precheck and dispatch
   validate identically and cannot drift (the P34 lesson).
2. **Command-list construction (§2 step 3)** — the broker refuses to build a receive-pack
   command line whose `new-oid` is the zero-oid (delete) or that carries any force capability.
   Force is **never expressible** because the broker constructs the line from
   `{old-oid (advertised), new-oid (frozen), refname}` with a fixed capability set — there is no
   code path that emits a force update.

This is **unreachable even via a human confirm** (a human confirms a specific push, not a
license to rewrite history) because both layers run inside the transfer, after the confirm burn,
with no operator-supplied bypass.

---

## §6. WIRING GAPS the DESIGN Left Open (highest-value output)

> These are the executable-plan blockers. Prior-phase researchers caught 3 (P32) / the
> SSRF-extension locus (P37) this way. Ranked by build risk.

**WG-1 — No "resolve-once, reuse-across-two-requests" primitive (BLOCKER-class for §1.5).**
`resolve_and_pin` (`http_request.rs:492`) resolves DNS, calls `vet_resolved` + `build_pinned_client`,
and returns ONLY a `reqwest::Client` — the vetted `SocketAddr` is **discarded**. Every
`do_pinned_*` helper calls it internally, so each request re-resolves. To freeze ONE IP across
the info/refs GET + receive-pack POST (the DNS-rebind close §1.5 mandates), the planner must add
a primitive that resolves+vets ONCE and hands both requests the SAME client. The DESIGN names
`build_pinned_client` as the reusable primitive, **but it is a private `fn`** (as are
`vet_resolved`, `resolve_and_pin`). **Fix:** widen `build_pinned_client` + `vet_resolved` to
`pub(crate)` and add a `pub(crate) async fn resolve_and_vet(host) -> Result<SocketAddr>`
(extracting the resolve+vet half of `resolve_and_pin`), then the git-push module builds ONE
client via `build_pinned_client(host, addr)` and issues both requests through it. Do NOT reuse
`invoke_pinned_post` (A7) — it re-resolves.

**WG-2 — `run_launcher` cannot capture a binary packfile (BLOCKER-class for pack gen).**
`run_launcher` (`process_exec.rs:402`) returns `(ExitStatus, String)` via
`String::from_utf8_lossy(&combined)` (`:490`), **merges stdout+stderr**, and **feeds no stdin**.
A PACK is binary; from_utf8_lossy corrupts it and the merged stderr pollutes it. **Fix:** add a
sibling confined-spawn variant, e.g.
`run_launcher_capture_bytes(..., stdin: Option<&[u8]>) -> Result<(ExitStatus, Vec<u8> /*stdout*/, Vec<u8> /*stderr*/)>`
under the SAME rlimits→Landlock→seccomp stack + `env_clear` + timeout + byte-cap + `kill_on_drop`
(refactor the shared machinery; do not fork it). This is the single largest new-code item after
the pkt-line glue. `git rev-parse` (§2 step 2) can still use the existing String `run_launcher`.

**WG-3 — git.push dispatch model (auto-Allowed vs github.pr-style always-gated) is UNPINNED.**
The DESIGN pins confirm-release on a TAINTED remote/refspec but is silent on a CLEAN
(untainted-from-trusted-intent) push. `github.pr`, even on an **Allowed** decision, stands
behind a live auth-grant and never auto-POSTs (`server.rs:1355`). git.push is equally
external+irreversible, and the §1.6 payload-provenance summary only surfaces on the CONFIRM path
— so a clean-remote push that auto-dispatches would push tainted-DERIVED commit content with no
human view of the payload. **Recommend:** mirror github.pr — gate EVERY git.push behind human
authorization (a grant or a mandatory confirm) so the payload summary is always surfaced; this
also makes the LIVE-05 happy-path composition explicitly human-gated (as github.pr already is).
**Flag for discuss-phase / user confirm** — it changes whether server.rs gets an
auto-dispatch arm or a grant-gated one. `[ASSUMED]` that github.pr-parity is intended; verify.

**WG-4 — Effect-ontology field mismatch.** `IrreversibleEffect::GitPush { remote, branch }`
(`effect.rs:27`) uses `branch`; the sink args + DESIGN §1.3 use `refspec`. Reconcile: either
rename the field to `refspec` or map (a refspec's `<dst>` → branch). Minor but a real
inconsistency that a plan touching the ontology must resolve.

**WG-5 — Registration couplings beyond the sink tables.** git.push is **absent from
PRODUCTION_SINKS** (`runtime-core/policy.rs:34`) — must be added or `broker_default()` PolicyDenies
it; the negative test `policy.rs:555` (`!permits_sink("git.push")`) must flip. And
`llm-planner/src/lib.rs:599-605` asserts `git.push => UnknownSink` — an easy-to-miss coupling if
the planner learns the sink. Enumerate all three in the executor-registration plan.

**WG-6 — old-oid provenance + create-vs-delete zero-oid ambiguity.** The command-list old-oid
must come from the **frozen info/refs advertisement** (the remote's current state), never the
local repo — else a lying old-oid enables force-ish behavior. Precise gate: `new-oid == zero-oid`
⇒ **delete** (DENY); `old-oid == zero-oid` (ref not advertised) ⇒ **create** (ALLOW). The
structural denial (§5) must key on **new-oid==zero only**, distinguishing it from a legitimate
create.

**WG-7 — new-oid freeze thread (see §4).** The anti-TOCTOU freeze requires the new-oid be
captured into the pending-confirmation snapshot at Block time and re-verified at dispatch. The
DESIGN states the principle but does not map it to the frozen-args mechanism — the plan must
thread it explicitly (new snapshot field, MAC coverage, dispatch-time equality refusal).

**WG-8 — no per-file taint-provenance renderer for a commit range.** `render_block_display`
(`confirmation.rs:570`) renders single-arg taint labels; there is no helper that, given a commit
range, lists the changed files and flags those with untrusted-derived content. §1.6/GIT-03 needs
one — net-new rendering reading value-store provenance.

**WG-9 — mock harness has no `git-receive-pack` endpoint.** `scripts/mock-github/server.py`
answers only `POST /repos/*/pulls` (404 everything else). The live proof needs a mock serving
`GET /info/refs?service=git-receive-pack` (a valid pkt-line advertisement) + `POST
/git-receive-pack` (consume the pack, return `unpack ok` report-status) — ideally backed by a
real bare repo so the pushed pack is genuinely accepted. Also **no git.push host-allowlist is
pinned**: recommend a distinct `GIT_PUSH_HOST_ALLOWLIST` (empty in release; the
`github-mock.caprun.test` host only under the `mock-egress-ca` feature), mirroring
`WRITE_HOST_ALLOWLIST` (A8) — a GET-readable or POST-writable host is not implicitly
push-target-able.

---

## §7. HYG-01 Supply-Chain Re-Run Command Set

Zero new crates are needed (pure Rust glue + already-present reqwest/rustls-ring/webpki-roots),
so Gate 5 should stay green with **no** transport change. The re-run CONFIRMS that:

```bash
# Run AFTER the git.push transport code lands (enumerates any dep that crept in):
cargo tree --workspace -i aws-lc-rs      # EXPECT: "did not match any packages" (absent)
cargo tree --workspace -i openssl-sys    # EXPECT: present ONLY via lettre (native-tls); NEVER via a reqwest path
cargo tree --workspace -i ring           # EXPECT: present — the sanctioned pure-Rust provider
cargo tree --workspace -i reqwest        # EXPECT: unchanged version =0.13.4, no new features
# The shipped gate already encodes the aws-lc-rs + openssl-sys-via-reqwest asserts:
bash scripts/check-invariants.sh         # Gate 5 (:211-233), Gate 4b (:180-189) must PASS
```

Plus the two HYG-01 hygiene items: (a) broaden `check-invariants.sh` Gate 4b (`:180-189`) to a
workspace-wide grep (not just brokerd) that `mock-egress-ca` is never a default feature; (b) add
a **feature-OFF guard step** in `compose-verify.sh` (a build/test run with NO `mock-egress-ca`
feature to prove the mock host/anchor is absent from release builds). If any new dep IS added it
MUST use `rustls-no-provider` + an explicit ring `CryptoProvider` or Gate 5 fails (the
resolver-3 feature-unification lesson, v1.8 Phase-37 MAJOR).

---

## §8. Suggested Plan Breakdown (mirrors Phase 43: 4 plans / 3 waves)

**Wave 1 (parallel — no cross-dependency):**

- **44-01 — Executor-TCB registration + structural denial (no network).**
  `KNOWN_SINKS` git.push row `{remote,refspec}` (`sink_schema.rs`); `sink_effect_class
  git.push=>CommitIrreversible` explicit + `_ =>` backstop; routing-sensitive `{remote,refspec}`,
  content-sensitive none, `expected_role None` (`sink_sensitivity.rs`); add to `PRODUCTION_SINKS`
  + flip the `policy.rs:555` negative test; reconcile `effect.rs GitPush` field (WG-4); update the
  `llm-planner:605` UnknownSink test (WG-5); the broker `validate_git_refspec` value-gate (§5,
  mirror `validate_write_method`). All host-portable unit tests. **Requirements:** GIT-03 (I2
  registration), GIT-02 (structural denial).
- **44-02 — Broker git-push egress module (`sinks/git_push.rs`).**
  pkt-line encode/decode + advertisement + report-status parsers (§3, host-portable); the
  frozen-IP two-request client (WG-1: widen `build_pinned_client`/`vet_resolved` to `pub(crate)`
  + add `resolve_and_vet`); the WG-2 binary-capable confined-spawn variant; confined-child pack
  generation via `git pack-objects` (§2, reuse git.commit's neutralization A14); broker-env
  credential (`CAPRUN_GIT_PUSH_TOKEN`) + opaque/scrubbed two-phase audit (clone `http_write.rs`
  A9); the distinct `GIT_PUSH_HOST_ALLOWLIST` (WG-9). Linux-gated socket/pack legs, macOS stubs.
  **Requirements:** GIT-02.

**Wave 2 (depends on 44-01 + 44-02):**

- **44-03 — Dispatch + confirm-release wiring.**
  Extend the Step-4.75 entry-guard (A19) with `git.push`; add Step-4.8d `prepare_git_push`
  precheck (A20); add the Step-7 async dispatch arm folding every failure into a terminal
  `git_push_succeeded`/`_failed` FIRST (A21, P33/P34); the server.rs Allowed/grant-gated arm per
  the WG-3 decision (A22); the new-oid freeze thread (WG-7, §4) + the commit-range/taint-provenance
  confirm renderer (WG-8, A23). **Requirements:** GIT-02, GIT-03.

**Wave 3 (depends on 44-03):**

- **44-04 — Differential acceptance + mock + HYG-01.**
  Extend `scripts/mock-github/server.py` with `git-receive-pack` (WG-9, backed by a bare repo);
  a Linux live proof asserting frozen-IP + net-denied-child + unbroken audit-DAG taint chain +
  `verify_chain`; the **differential** tainted-remote-Blocks vs clean-Allowed leg (taint the sole
  variable); the credential/remote-URL absence assertion covering value-store + audit chain +
  **broker logs** (§1.4 MINOR-4, LIVE-06 leg 5); the HYG-01 re-run (§7) + compose-verify
  feature-OFF guard + Gate 4b workspace-wide broadening. **Requirements:** GIT-02, GIT-03, HYG-01.
  (LIVE-05/06 full composition is Phase 46; 44-04 proves the git.push leg in isolation.)

**Dependency structure:** Wave1 {44-01 ∥ 44-02} → Wave2 {44-03} → Wave3 {44-04}. Identical shape
to Phase 43 (executor-TCB ∥ egress-module → dispatch/confirm → acceptance).

**Safety-valve reminder (§1.9):** if the orchestrator-owned adversarial trace on this phase's
plans proves the broker-performed transfer does not actually keep the child net-denied while
pinning the destination, GIT-02/03 DEFER (disclosed, sign-off-gated, auto-descope from
LIVE-05/06) — never a net-allowed child, never a silent drop. A mid-build transport/trust-posture
pivot RE-RUNS the DESIGN-18 trace (`DESIGN-GATE-RECORD-v1.9.md`).

---

## Project Constraints (from CLAUDE.md)

- **TCB is Rust** — all git.push glue in `crates/brokerd`/`executor`/`runtime-core` + `sandbox`
  (unchanged). No Python in the TCB (the mock endpoint is test-harness only).
- **Effect path is locked** — git.push is a `PlanNode { sink:"git.push", args:[remote, refspec] }`;
  NEVER a raw `EffectRequest` (Gate 1). I2 runs unconditionally in `submit_plan_node`.
- **Linux-only security tests** — the socket + `git pack-objects` legs are `#[cfg(target_os =
  "linux")]`; verify via `scripts/compose-verify.sh` (Phase 16+ mandate), NOT bare `docker run
  rust:1`. macOS shows 0-passed for the gated legs — expected, not a gap.
- **`./scripts/check-invariants.sh` runs before any code** and Gate 5/4b re-run for HYG-01.
- **v0 DONE discipline carries:** genuine propagated taint (a real DAG edge), never stapled at
  the sink; terminal audit event BEFORE terminal state on every confirm-release failure leg.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | git.push mirrors github.pr's always-on grant/confirm gate (clean pushes are human-gated, not auto-dispatched) | WG-3 | If wrong, server.rs needs an auto-dispatch arm instead and a clean push proceeds with no payload view — changes the dispatch plan materially. **Confirm with user.** |
| A2 | `git pack-objects --revs --stdout --thin` fed `<new>\n^<old>` yields a receive-pack-acceptable pack | §2 | If the remote rejects the thin pack shape, may need `--no-thin` or `git bundle`/send-pack fallback. Verify on the mock live proof. |
| A3 | Omitting `side-band-64k` from advertised caps yields a plain-band report-status (no demux needed) | §3 | If a target remote forces side-band, the parser needs band demux. Low risk on the mock; flagged for real remotes. |
| A4 | Zero new crates suffice (pkt-line hand-rolled, no `gix`/`git2`) | §2/§7 | If hand-rolled pkt-line proves too costly, a crate would trigger the HYG-01 ring-only recipe. Research rates hand-roll FEASIBLE. |

*All other claims are VERIFIED against live code (`file:line` re-read this session) or CITED to
the locked DESIGN/research.*

---

## Sources

### Primary (HIGH confidence — live code re-read this session, HEAD `89cab31`)
- `crates/brokerd/src/sinks/http_request.rs` — resolve-and-pin, build_pinned_client, invoke_pinned_post, invoke_http_write, validate_write_method (anchors A1–A8)
- `crates/brokerd/src/sinks/http_write.rs`, `github_pr.rs`, `process_exec.rs`, `git_commit.rs` (A9–A11, A14)
- `crates/sandbox/src/seccomp.rs:147`, `cli/caprun-exec-launcher/src/main.rs` (A12–A13)
- `crates/executor/src/{sink_schema,sink_sensitivity,lib}.rs` (A15–A16)
- `crates/runtime-core/src/{policy,effect}.rs`, `crates/brokerd/src/confirmation.rs`, `server.rs` (A17–A23)
- `scripts/check-invariants.sh`, `scripts/compose-verify.sh`, `scripts/mock-github/server.py` (A24–A25)

### Locked design inputs (CITED — cleared the Phase-41 gate; not re-decided here)
- `planning-docs/DESIGN-v1.9-egress-policy.md §1` (§1.1–§1.9) + Round-1 Amendments (MINOR-3/4/5)
- `.planning/research/GIT-PUSH-EGRESS.md` (Candidate (b) FEASIBLE, zero new crates)
- `.planning/ROADMAP.md` Phase 44 (5 Success Criteria + safety-valve), `.planning/REQUIREMENTS.md` (GIT-02/03, HYG-01, Out-of-Scope)
- git smart-HTTP protocol: `gitprotocol-http`, `gitprotocol-pack` (CITED via GIT-PUSH-EGRESS.md sources)

## Metadata

**Confidence breakdown:**
- Live-code anchors: HIGH — every `file:line` re-verified against HEAD this session.
- Pack-gen realization: HIGH — reuses git.commit's confined-child muscle; the one gap (binary capture) is precisely located (WG-2).
- pkt-line glue effort: MEDIUM — the research's standing MEDIUM; sized honestly (~180–280 LoC pure + Linux-gated legs).
- Wiring gaps: HIGH — 9 enumerated, each with a concrete fix and `file:line`.

**Research date:** 2026-07-18
**Valid until:** 2026-08-01 for anchors (re-verify line numbers if Phase 44 code lands many commits later — this project's own convention); design inputs stable through the milestone.
