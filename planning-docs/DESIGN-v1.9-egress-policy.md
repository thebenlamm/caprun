# DESIGN — Authorized Egress + Policy & Audit Surface: `git.push`, `http.request` WRITE, the policy↔I2 boundary

**Milestone:** v1.9 — Authorized Egress + Policy & Audit Surface
**Phase:** 41 (Design Gate) — blocks all `crates/{executor,brokerd,sandbox,runtime-core}`
and `cli/` TCB code for this milestone
**Status:** Draft → pending a fresh, **non-self, orchestrator-owned** adversarial
code-trace (DESIGN-18) to be recorded in `planning-docs/DESIGN-GATE-RECORD-v1.9.md`.
This doc is authored by a `gsd-executor`; the executor does **not** run or self-perform
that trace (gsd-executors have no Agent tool — §9).
**Author date:** 2026-07-18
**Grounding:** `.planning/research/GIT-PUSH-EGRESS.md` (the AUTHORITATIVE `git.push`
mechanism input — pins candidate (b)), `.planning/REQUIREMENTS.md` (DESIGN-17/18,
GIT-02/03, HTTP-W-01, POLICY-01/02/03, HYG-01, LIVE-05/06), and the v1.8 doc
`planning-docs/DESIGN-git-github-http-sinks.md` (whose §2 / §2.5 / §2.7 / §9 carry
forward here **by reference**). Every `file:line` below traces to a direct code read
this session; re-verify if Phases 42-46 begin many commits later, per this project's own
convention.
**Requirements:** DESIGN-17 (this doc) → enables POLICY-01/02/03 (Phase 42),
HTTP-W-01 (Phase 43), GIT-02/03 + HYG-01 (Phase 44), SDK-01 + U1 (Phase 45),
LIVE-05/06 (Phase 46). DESIGN-18 is the gate that clears it (§9).

> **Design-gate discipline.** No `crates/executor` / `crates/brokerd` /
> `crates/sandbox` / `crates/runtime-core` or `cli/` code for any v1.9 surface may be
> written until this document clears a fresh, non-self, orchestrator-owned adversarial
> code-trace with every BLOCKER/MAJOR resolved — the unbroken caprun precedent (v1.0 P2,
> v1.2 P8, v1.3 P12, v1.4 P18, v1.5 P23, v1.6 P26, v1.7 P31, v1.8 P35). This doc pins
> **decisions**, not options. `git.push`'s network-from-a-confined-child path is the
> **riskiest surface in the project to date** — the v1.8 gate deferred it precisely because
> the fresh reviewer proved the original "net-allowed child" model unsound (BLOCKER-1). This
> doc pins the deferred-and-now-researched mechanism precisely enough that the reviewer can
> trace every claim against real code.

---

## §0. Purpose & Scope

**What this doc pins (DESIGN-17).** The TCB mechanism + fail-closed default for the three
v1.9 external surfaces, before any TCB code exists:

1. **`git.push`** (§1) — a **broker-performed git smart-HTTP transfer** (research
   candidate (b)): the broker plays the HTTP mover (reqwest resolve-and-pin, TLS
   terminates broker-side), the push child stays **fully net-denied** and does only
   local pack generation. The destination pin lives in the broker application layer that
   SEES the destination IP:port (`reqwest .resolve(host, pinned_ip)`), **never seccomp**.
   Closes the research's three adversarial attack points (credential leak; redirect/
   DNS-rebind pin-bypass; payload-vs-destination confirm TOCTOU). Effect-class
   `CommitIrreversible`.
2. **`http.request` WRITE (POST/PUT)** (§2) — a **distinct** write host-allowlist
   (separate from the shipped GET allowlist), taint-governed content-sensitive body under
   I2, routing-sensitive `url`, broker-env-only credential custody with response scrub, and
   **differential** acceptance.
3. **The policy↔I2 boundary** (§5), incl. **POLICY-03 binding** — policy is a **pre-I2
   narrowing gate** (which sinks/args are callable), refusing with a distinct
   machine-checkable policy-deny outcome; it can NEVER disable/override I2; I2 stays
   **HARDCODED** in the Rust TCB executor, unconditional on every policy-permitted call;
   the policy is bound by the broker at session creation from a trusted source outside the
   confined worker's reach (F1 containment reused verbatim from `cli/caprun/src/key.rs`),
   immutable, hash recorded as an audit-DAG event.

Plus: the **crypto-provider / supply-chain** decision (§3 — ring-only, ZERO new crates),
the **fail-closed defaults table** (§4), a **§-per-pitfall threat model** (§6),
**invariant preservation** (§7), the **new-symbol summary** (§8), the
**orchestrator-owned adversarial-trace gate** (§9), and the **acceptance predicate** (§10).

**Carried forward from v1.8 BY REFERENCE (not restated wholesale).** The following v1.8
sections are load-bearing for v1.9 and are extended, not re-derived, here:

- **v1.8 §2** (`git.push` = Pattern-B local dispatch, child net-denied, broker-mediated
  egress; FORK-1 re-decided). v1.9 §1 realizes its deferred Open-Item-1 mechanism.
- **v1.8 §2.5** (captured-output credential scrub / opaque-payload discipline). v1.9 §1.4
  and §2 inherit it verbatim.
- **v1.8 §2.7** (pushed-payload visibility at confirm — commit range + tainted-file
  provenance summary). v1.9 §1.6 inherits it and adds the anti-TOCTOU freeze.
- **v1.8 §9** (P33/P34 confirm-release audit-gap discipline: terminal audit event before
  terminal state, `prepare_*` precheck, entry-guard allow-list extension). v1.9 §1.7
  applies it to `git.push` (and, if confirm-releasable, http-write).

**Two shipped dispatch patterns, unchanged; no new pattern, no raw effect-request path.**

- **Pattern A — in-broker / broker-helper network egress.** The shipped
  `crates/brokerd/src/sinks/http_request.rs` resolve-and-pin GET egress
  (`invoke_http_get`, `http_request.rs:414`) + the already-present authenticated POST
  (`invoke_pinned_post`, `http_request.rs:506`) are the exemplars. `git.push`'s network
  leg (§1) and `http.request` WRITE (§2) both live here — broker-resident, never
  confined-worker-resident.
- **Pattern B — broker-spawned confined child.** `crates/brokerd/src/sinks/process_exec.rs`
  `run_launcher` (`process_exec.rs:402`) with `env_clear()` (`process_exec.rs:422`) +
  minimal `SAFE_EXEC_PATH` (`process_exec.rs:385`). `git.push`'s **local pack-generation
  child** rides this unchanged from `git.commit`; its seccomp net-deny is identical to
  `git.commit`'s (§1.2).

**Locked terminology (unchanged):** `Intent`, `Session`, `Planner`, `Worker`, `Broker`,
`Adapter`, `Effect`, `Artifact`, `Event`. `ExecutionContext` stays internal-only. Nothing
here introduces new public-API vocabulary.

**No TCB code this phase.** This doc lives entirely under `planning-docs/`. The git diff
for Plan 41-01 touches only `planning-docs/DESIGN-v1.9-egress-policy.md`.
`scripts/check-invariants.sh` stays green (its prose under `planning-docs/` trips no Gate
that scans `crates/` or `cli/`).

---

## §1. `git.push` egress — broker-performed smart-HTTP transfer, child fully net-denied (GIT-02/03)

This realizes v1.8 §2 Open-Item-1 with the mechanism the research pins as **FEASIBLE**
(`.planning/research/GIT-PUSH-EGRESS.md`, "PIN: Candidate (b)"). A third deferral is **not**
warranted — but the safety-valve (§1.9) remains, disclosed and sign-off-gated, if the
adversarial trace proves the mechanism unsound.

### 1.1 The pinned mechanism — candidate (b), the git-native split

`git.push`'s network leg is performed by the **broker**, over the SAME already-shipped
Pattern-A egress path the design gate already blessed as sound (application-mediated, not
kernel-syscall-filtered): `reqwest =0.13.4` + `rustls 0.23` (ring) + `webpki-roots` +
SSRF resolve-and-pin (`http_request.rs`). Git's smart-HTTP push is a well-specified
**two-request** exchange the broker drives directly (`gitprotocol-http`):

1. `GET $URL/info/refs?service=git-receive-pack` → pkt-line ref advertisement + `report-status`.
2. `POST $URL/git-receive-pack` (`Content-Type: application/x-git-receive-pack-request`),
   body = pkt-line command-list (`<old> <new> <ref>`) + `"PACK" <binary>`.

**DECISION — the git-native split.** The broker plays the **HTTP-mover role** (reqwest,
TLS broker-side, `.resolve(host, pinned_ip)` = the pin) driving **`git send-pack
--stateless-rpc` running as the net-denied child** for pure-local delta + pack computation
(no socket). TLS and the entire network leg live **broker-side**; the child never opens an
`AF_INET`/`AF_INET6` socket. (Implementation may alternatively generate the receive-pack
body directly from the workspace `.git` in the broker — the broker has fs access — but the
security posture is identical: the child is net-denied either way; Phase 44 picks the
realization under the §1.9 constraint.)

The child's **local dispatch is `git.commit`'s Pattern-B path unchanged** (v1.8 §1.1 /
§2): broker spawns the `caprun-exec-launcher`, which self-confines then `execve`s `git`;
the confined worker never `execve`s git (its own filter denies `execve` unconditionally,
`seccomp.rs:65`). Effect-class = pinned **`CommitIrreversible`** (matching the locked
`IrreversibleEffect::GitPush` ontology, v1.8 §2.1).

### 1.2 Child stays FULLY net-denied — the pin is application-layer, NEVER seccomp

**DECISION (carries v1.8 §2.1/§2.3 FORK-1, the BLOCKER-1 correction — LOCKED).** The
`git.push` child's seccomp filter is the exec-child filter **verbatim** — `exec_child_filter`
(`sandbox/src/seccomp.rs:147`) denies `socket(AF_INET)` and `socket(AF_INET6)` with `EPERM`
(`seccomp.rs:163-188`), identical to `git.commit`. There is **NO seccomp relaxation**, because
seccomp-bpf provably cannot pin a destination: it gates syscall numbers + scalar registers
only, and `connect()`'s destination is a `struct sockaddr *` behind a pointer it cannot
dereference; `socket()` exposes only the address *family*. The only relaxation seccomp could
make is all-or-nothing "stop denying `AF_INET`," which grants **arbitrary egress** to a
credential-bearing child — the exact exfiltration primitive the taint model exists to defeat.
Landlock cannot help either (`LANDLOCK_ACCESS_NET_CONNECT_TCP` needs ABI V4 / kernel 6.7 above
the 5.13 floor, and filters by port only). This is a caprun anti-requirement
(`.planning/REQUIREMENTS.md` Out of Scope: "Seccomp-based git.push destination pinning",
"Net-allowed git.push child").

**The destination pin lives in the broker application layer that SEES the IP:port.** It is
`reqwest.Client::builder().resolve(host, pinned)` (`http_request.rs:337`) inside
`build_pinned_client` (`http_request.rs:333`), fed the exact SSRF-vetted IP from
`vet_resolved` (`http_request.rs:313`) via `resolve_and_pin` (`http_request.rs:431`). The
same locus already enforces the GET/POST pin live-proven on Linux (Phase 40). **The pin is
NEVER claimed of seccomp.** seccomp's role for `git.push` is purely the fail-closed backstop
(§1.8), not the pin.

### 1.3 Remote/refspec from TRUSTED intent; `--force`/deletion hard-denied

**DECISION.** The push **remote URL + refspec are captured from the TRUSTED intent at
session creation** and passed explicitly to the transfer leg — **NEVER** resolved from the
untrusted repo's `.git/config` (which a prompt-injected worker could rewrite). This mirrors
`email_smtp.rs`'s D-04 endpoint sourcing (v1.8 §2.2): routing identity comes from trusted
config, never from block-time-writable state. `remote` + `refspec` are **I2-gated sink
args** (tainted → Block, §4).

`--force` / `--force-with-lease`, `:refspec` ref-deletion, and `+`-prefixed force-refspec
are **hard-denied by construction** at arg validation — unreachable **even via a human
confirm** (a human confirms a *specific* push, not a license to rewrite history). This
carries v1.8 §2.4 verbatim onto the broker-driven path: the broker MUST refuse to build a
receive-pack command-list containing a delete (`<new>` = zero-id) or a force update it was
not constructed to permit.

### 1.4 Attack point (i) — credential leak (carries v1.8 §2.5)

The push token now transits the **broker's own HTTP client** (it plays the mover), so the
broker holds the credential AND sees the full HTTP exchange — the highest-value leak surface.
**DECISION (carries v1.8 §2.5 verbatim):**

- The credential lives in **broker-local env ONLY** — never a `ValueNode`, plan-node arg,
  audit-DAG literal, the confined worker, or the planner sidecar. It is supplied to the
  transfer as the `Authorization` header value on the reqwest POST (the `bearer`/basic slot
  of `invoke_pinned_post`, `http_request.rs:506-517`) — set on the request, never persisted.
- If the push instead drives `git` for the network leg in any variant, the credential is the
  ONE explicitly-injected non-`SAFE_EXEC_PATH` env var (`extra_env`, `process_exec.rs:394`)
  scoped to that child alone, riding `run_launcher`'s `env_clear()` discipline
  (`process_exec.rs:422`) proven by `run_launcher_env_clear_prevents_broker_secret_leak`
  (`process_exec.rs:850`).
- **Captured child/transport output follows the opaque/scrub discipline, NOT
  `process.exec`'s mint-the-output default.** A network exchange routinely echoes
  endpoint/credential-adjacent material a local commit never does (proxy-auth `407`,
  redirect/URL echoes on `401`). So the push's captured stdout/stderr and any HTTP response
  body is **either not minted at all** (only a broker-side opaque `git_push_succeeded`/
  `_failed` event carrying `effect_id`) **or scrubbed of any `https://…@…` userinfo /
  `Authorization:` / token / proxy-auth substring before minting**. A regression test MUST
  assert **no credential or remote-URL substring survives into the value store or the audit
  chain** (LIVE-06 leg 5, the post-push credential-absence assertion).

### 1.5 Attack point (ii) — destination-pin bypass via redirect / DNS-rebind

The two-request exchange opens a rebind/redirect window the single-request GET does not.
**DECISION:**

- **One frozen resolved IP across BOTH requests.** The IP vetted and pinned for the
  `info/refs` GET is **reused (frozen)** for the `git-receive-pack` POST — there is **no
  re-resolve between requests**. `resolve_and_pin` (`http_request.rs:431`) already returns a
  client bound to the exact vetted IP set (its doc: "The resolved IPs are the EXACT set
  vetted and pinned (no re-resolve later — DNS-rebind TOCTOU close)"). Phase 44 MUST build
  the POST client from the SAME pinned `SocketAddr` the GET used — not a fresh
  `resolve_and_pin` call — so a DNS answer that flips between requests cannot move the POST.
- **POST 3xx redirects refused.** `build_pinned_client` sets
  `.redirect(reqwest::redirect::Policy::none())` (`http_request.rs:335`) — a followed
  redirect = arbitrary egress = the exfil primitive. This governs the **POST**, not just the
  GET: a `git-receive-pack` 3xx (renamed repo / org redirect) MUST be refused, never
  followed. The shipped `invoke_pinned_post` (`http_request.rs:525-542`) already builds its
  client via the SAME `resolve_and_pin` → `build_pinned_client` path, so redirect-none is
  already in force for the POST; Phase 44 MUST NOT introduce a redirect-following client for
  the git leg.

### 1.6 Attack point (iii) — payload-vs-destination confirm TOCTOU (carries v1.8 §2.7)

I2 mediates values routed into sink args; `git.push`'s args are `remote` + `refspec` only,
so absent a mechanism the human would authorize a destination without seeing what leaves the
boundary. **DECISION (carries v1.8 §2.7 + adds an anti-TOCTOU freeze):**

- At the `git.push` confirm prompt the human is shown (a) the pushed commit range / branch,
  and (b) a **provenance summary of the pushed diff flagging any file whose content derives
  from untrusted taint** (via the value-store provenance the executor already tracks). The
  **payload**, not just `remote`/`refspec`, is what the human authorizes.
- **Anti-TOCTOU freeze:** the pack is **generated/frozen from the confirmed commit range
  at-or-after confirm** — there is no window in which a compromised worker rewrites the tree
  between confirm and pack generation. The provenance summary shown at confirm MUST describe
  the **same bytes** that get POSTed. Because the broker (not the worker) drives pack
  generation from the confirmed commit range at-or-after confirm, the worker cannot
  substitute tree content post-confirm.
- Accepted residual (carries v1.8 §11): this surfaces provenance for human judgment; it does
  not by itself Block untainted-but-sensitive committed content — that remains the human's
  call at confirm, consistent with the confirmation model.

### 1.7 Confirm-release audit-gap discipline (carries v1.8 §9 — P33/P34)

`git.push` is `CommitIrreversible` + confirm-releasable. **DECISION (carries v1.8 §9
verbatim):** the confirm-release path writes the **TERMINAL AUDIT EVENT before the terminal
state** — never a terminal state (`Confirmed` / `confirm_granted`) before the terminal event
that justifies it. Phase 44 MUST add a `prepare_git_push` precheck run at `confirm()`'s
Step 4.8-class point **before** Step 5 appends `confirm_granted` (`confirmation.rs:928-941`)
and Step 6 burns the one-shot — folding every fallible pre-transfer leg through the single
terminal-event branch, exactly as `prepare_process_exec` (`confirmation.rs:864`,
`process_exec.rs:362`) and the `github.pr` Step 4.8b precheck (`confirmation.rs:872-903`)
already do. The `confirm()` Step 4.75 entry-guard allow-list of confirm-releasable sinks
(`confirmation.rs:825-846`) MUST be extended to admit `git.push` — a confirm-releasable sink
absent from that allow-list is denied at the guard (fail-closed), so the extension is
required, not optional. A regression test MUST assert **no dangling `confirm_granted` without
a terminal event** on any pre-transfer failure leg. This is the recurring MAJOR audit-gap
class a passing verifier + green gates missed twice (v1.7 P33 file.write, P34 process.exec)
and only the fresh adversarial trace caught.

### 1.8 `send-pack` self-egress — the seccomp fail-closed backstop

If `git send-pack --stateless-rpc` (or any git subprocess in the child) ever attempts its own
`connect()`/DNS, the child's seccomp `socket(AF_INET/AF_INET6) → EPERM`
(`seccomp.rs:163-188`) is the **fail-closed backstop** — the transport MUST delegate all
network to the broker mover (it does; `--stateless-rpc` reads request bodies on stdin and
writes to stdout, leaving the socket to the HTTP mover). This is a backstop, not the pin: the
pin is §1.2's application-layer resolve; the backstop guarantees a mover-bypass attempt fails
closed rather than reaching an arbitrary host.

### 1.9 Safety-valve — disclosed, sign-off-gated deferral (never a silent drop)

**DECISION (carries v1.8 §2.1 HARD CONSTRAINT + GIT-02/LIVE-05 safety-valve).** If the
orchestrator-owned adversarial code-trace (§9) proves that no fully-unprivileged
destination-pinning mechanism is sound under review — i.e. the broker-performed transfer
does not actually keep the child net-denied while pinning the destination in a locus that
sees it — then `git.push` (GIT-02/GIT-03) **DEFERS** rather than shipping with any
arbitrary-egress fallback. The deferral is:

- **Disclosed** — recorded as a milestone gap in `DESIGN-GATE-RECORD-v1.9.md`, not silently
  dropped;
- **Sign-off-gated** — requires explicit user sign-off (never an orchestrator-autonomous
  drop, LIVE-05 `[rev: M6/n1]`);
- **Scoped** — the `git.push` leg auto-descopes from LIVE-05/06; the other three tracks
  (POLICY, http-write, SDK/viewer) proceed untouched (that is why the roadmap splits
  GIT-02/03 from HTTP-W-01 by blast radius).

Shipping arbitrary child egress is **never** an option — that is the exact primitive the
BLOCKER-1 correction exists to prevent.

---

## §2. `http.request` WRITE (POST/PUT) egress — distinct allowlist, taint-governed body (HTTP-W-01)

v1.8 shipped `http.request` GET (Pattern A) and, for `github.pr`, an authenticated
`invoke_pinned_post` (`http_request.rs:506`). v1.9 generalizes WRITE into a first-class,
I2-governed sink. **DECISIONS:**

### 2.1 A DISTINCT write host-allowlist (separate from the GET allowlist)

The shipped read/GET allowlist is `HOST_ALLOWLIST = ["api.github.com"]`
(`http_request.rs:101`), checked by `is_host_allowlisted` (`http_request.rs:119`). **A host
being GET-readable does NOT imply it is POST/PUT-writable** (`[rev: m1]`). Phase 43 MUST
introduce a **separate WRITE host-allowlist constant** (e.g. `WRITE_HOST_ALLOWLIST`) that the
WRITE path checks — NOT reuse `HOST_ALLOWLIST`. (Note: the existing `invoke_pinned_post`
currently gates on `HOST_ALLOWLIST` at `http_request.rs:513`; the generic `http.request`
WRITE sink MUST gate on the distinct write-allowlist instead, and `github.pr`'s fixed
`api.github.com` base is orthogonal broker-owned trusted config.) The write-allowlist is a
broker-local trusted-config **security property** (an operator-surfaced deployment constant),
never runtime-configurable from a plan node / `ValueNode` / audit DB — same status as
`HOST_ALLOWLIST`.

### 2.2 Body taint-governed + content-sensitive under I2; `url` routing-sensitive

The request **BODY is content-sensitive under I2** — a value assembled from untrusted
content (an `HttpRaw`/`ExecRaw`/`ExternalUntrusted`-tainted string) routed into the body
**deterministically Blocks** under the UNMODIFIED `submit_plan_node` collect-then-Block loop,
exactly like an `email.send` body or a `github.pr` `title`/`body` (v1.8 §4.4). This is the
marquee exfiltration defense for write-egress: without it a prompt-injected worker could
smuggle secrets out through a POST body. Taint is **genuinely propagated** (a real DAG edge),
never stapled at the sink. The **`url` is routing-sensitive** (it determines where the write
lands), I2-gated (tainted → Block).

### 2.3 Reuse the shipped SSRF resolve-and-pin verbatim

The WRITE path rides the SAME defense-in-depth as the GET (`http_request.rs`): `validate_url`
(`http_request.rs:135` — rejects `userinfo@`, non-`https`, explicit ports, IP-encoding
tricks) → write-allowlist gate BEFORE any resolve (fail-closed, §2.1) → `resolve_and_pin`
(`http_request.rs:431`) → `vet_resolved` → `ssrf_check` (`http_request.rs:175` — denies
loopback/RFC1918/link-local/metadata/CGNAT/ULA/IPv6-mapped/transition ranges) → pin to the
vetted IP with `redirect(Policy::none())` (`http_request.rs:335`). **No classifier is
re-implemented** — the WRITE path invokes the same functions. TLS anchors = the ring +
`webpki-roots` config (`ring_webpki_tls_config`, `http_request.rs:398`), `env_clear()`
hermetic (§3).

### 2.4 Broker-env-only credential custody + response scrub

Any write credential lives in **broker-local env ONLY** — never a `ValueNode`, plan-node
arg, audit-DAG literal, the confined worker, or the planner sidecar (`[rev: M1]`, same D-04
custody as `email_smtp.rs` and §1.4). The captured response is **scrubbed of credential
material (or not minted)** before it can reach the value store or audit chain — mirroring
§1.4 / v1.8 §2.5. If the WRITE response is minted as an inbound value at all, it is minted
via `mint_from_http` (untrusted-on-arrival, `HttpRaw` + `ExternalUntrusted`, session
demotion) — never stapled — so a response routed into a downstream sensitive sink Blocks on a
real DAG edge.

### 2.5 Differential acceptance (`[rev: M4]`)

Acceptance is **differential**, not "not blocked": the tainted-body-Blocks leg and the
clean-body-Allowed leg are **identical in host/url/method/policy** (taint is the sole
variable), and the clean leg is confirmed to have **actually delivered the body to the mock
endpoint on real Linux** (the mock records receipt). A block-everything I2 regression cannot
pass this — a passing run requires the clean body to arrive AND the tainted body to Block,
attributable to I2 specifically (LIVE-06).

---

## §3. Crypto provider + supply-chain — ring-only, ZERO new crates (HYG-01)

**DECISION.** v1.9 adds **ZERO new crates**. `git.push` (§1) and `http.request` WRITE (§2)
reuse the already-shipped `reqwest =0.13.4` + `rustls 0.23` (ring provider) + `webpki-roots`
+ SSRF resolve-and-pin stack already in `brokerd` (`http_request.rs:398-405`
`ring_webpki_tls_config` uses `rustls::crypto::ring::default_provider()`;
`http_request.rs:368-390` `egress_root_store` uses `webpki-roots::TLS_SERVER_ROOTS`). Per the
research: `cargo tree -p brokerd -i openssl-sys` = not-found; ring-only, no aws-lc-rs/openssl
C. The only new code is auditable Rust protocol glue (pkt-line framing + the two HTTP
requests + `report-status` parse), well-specified by `gitprotocol-http` — no new dependency,
no external binary, no userns/netns.

**Gate 5 workspace-scoped absence re-run (HYG-01).** `scripts/check-invariants.sh` Gate 5
(`check-invariants.sh:211-233`) asserts, over the WHOLE workspace graph (NOT `-p brokerd` —
resolver-3 unifies features workspace-wide), that `cargo tree --workspace -i aws-lc-rs` and
`cargo tree --workspace -i openssl-sys` (via a reqwest path) are **absent**. This assertion
**RE-RUNS after the `git.push` transport dependency is chosen** (Phase 44), enumerating any
new transport deps — not just deps known at planning time. If a new dep IS added it MUST honor
the ring-only recipe (`rustls-no-provider` + an explicitly-supplied ring `CryptoProvider`),
or Gate 5 fails the build. This is the resolver-3 feature-unification lesson (v1.8 Phase-37
MAJOR: a sibling crate's `reqwest features=["rustls"]` silently pulled aws-lc-rs C into the
broker TCB). HYG-01 also broadens the Gate-4b never-default discipline
(`check-invariants.sh:180-189`) to a workspace-wide grep and adds a feature-OFF guard step in
`compose-verify.sh`.

---

## §4. Fail-Closed Defaults Table

Each row states the safe default when a precondition is absent/ambiguous. New v1.9 mechanisms
only; v1.8's table (its §8) carries forward for the shared GET/`github.pr`/SSRF rows.

| Sink arg / mechanism | Sensitivity | Default posture | Fail-closed behavior |
|---|---|---|---|
| `git.push` destination pin | routing | broker resolves ONCE, freezes the vetted IP across info/refs GET + receive-pack POST; pin in the app layer (`http_request.rs:337`), never seccomp | unresolvable / SSRF-range / mixed DNS answer → Deny; a re-resolve or redirect between requests → refused |
| `git.push` `remote` / `refspec` | routing (I2-gated) | from TRUSTED intent only, never repo `.git/config` | tainted → Block; not-from-trusted-intent → Deny; `--force`/deletion/`+`-force shape → hard Deny (unreachable even via confirm) |
| `git.push` credential custody | secret | broker-local env ONLY; `Authorization`/askpass to the transfer leg alone | ever appearing in a `ValueNode`/audit-literal/broker-log/worker → the design is violated; captured output not-minted-or-scrubbed |
| `git.push` captured output | untrusted | opaque event (`git_push_succeeded`/`_failed`, `effect_id` only) OR scrubbed before mint | any `https://…@…`/`Authorization:`/token substring surviving into value-store/audit → regression-test FAIL |
| `git.push` no-sound-mechanism | — | broker-performed transfer under review | if unsound under §9 trace → DEFER (disclosed, sign-off-gated); NEVER ship net-allowed child |
| `http.request` WRITE host | routing | **distinct** `WRITE_HOST_ALLOWLIST`, checked BEFORE any resolve | non-write-allowlisted host → Deny at the gate (a GET-readable host is NOT implicitly writable) |
| `http.request` WRITE `body` | content-sensitive (I2) | taint carrier, never re-minted clean | tainted → Block (collect-then-Block); unknown/missing → Deny at Step 0 schema gate |
| `http.request` WRITE `url` | routing-sensitive (I2) | resolve-and-pin; redirect-none | SSRF-range/redirect/`userinfo@`/non-https → Deny; tainted → Block |
| `http.request` WRITE credential + response | secret / untrusted | broker-local env only; response scrubbed or `mint_from_http`-tainted | credential in value-store/audit → violated; response stapled-clean → violated |
| session policy source (POLICY-03) | trust binding | bound by broker at session creation from a trusted path outside worker reach (F1) | policy path at-or-beneath workspace root, or unresolvable/absent → **refuse to run** (fail-closed, no session) |
| policy vs I2 (POLICY-02) | invariant | policy narrows which sinks/args are callable | policy PERMIT never disables I2 — a tainted sensitive arg on a permitted call still Blocks; policy can only add a Deny, never remove a Block |
| policy-deny outcome (POLICY-01) | structural | distinct machine-checkable policy-deny, separate from an I2 Block | sink/arg not permitted by policy → policy-Deny (distinct terminal tag), before/independent of I2 |

<!-- gsd:policy-section-pending (Task 2) -->
