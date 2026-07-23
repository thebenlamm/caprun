# Phase 12: Content, Adapter & Confirm-Binding Design Gate - Research

**Researched:** 2026-07-07
**Domain:** Security design (I2 executor extension + broker-mediated SMTP adapter + confirmation-release binding), Rust TCB
**Confidence:** HIGH (architecture/pitfalls — grounded in direct reads of this repo's own `crates/`); MEDIUM (SMTP crate/CRLF mechanics — grounded in direct reads of upstream source); LOW-MEDIUM (exact Mailpit HTTP API endpoint paths — not independently fetched, flagged as an assumption)

## Summary

This phase produces documentation only: a DESIGN doc (or paired docs) that gates all executor/TCB code for CONTENT-01/02, SMTP-01/02/03/05, and CONFIRM-03 before Phases 13-16 may write it. The single most important research finding is architectural, not a library choice: **the current executor's per-arg loop returns on the FIRST tainted routing-sensitive arg it finds** (`crates/executor/src/lib.rs`, Step 2), and `SinkBlockedAnchor`/`PendingConfirmation` are built around **one blocked arg per Block decision**. If CONTENT-01 is bolted onto Step 3 unchanged (mark content-sensitive taint, don't block), or if it blocks independently without changing this single-arg shape, the exact B1 failure mode (`DESIGN-REVIEW-v1.2-round1.md`) reincarnates for the body: a tainted recipient blocks first, gets confirmed, and the adapter directly invokes the sink using the FULL frozen `resolved_args` snapshot — silently sending a never-individually-confirmed tainted body along with it. D-02/D-12(a) explicitly names this risk; the fix is architectural (collect ALL sensitive+tainted args in one pass, Block once as a set, confirm releases the whole set), and this is the one finding the DESIGN doc absolutely must resolve explicitly, not merely acknowledge.

The second load-bearing finding is that `email.send`'s content-sensitivity classification (`is_content_sensitive`, `crates/executor/src/sink_sensitivity.rs`) **already exists and already returns `true`** for `subject`/`body`/`attachment` — CONTENT-02's "one hardcoded match arm" is NOT new code to write; it is the existing classification, whose *consequence* (Step 3 of `submit_plan_node`) changes from "mark for later Tier-4 review, fall through" to "Block, same as routing-sensitive." The DESIGN doc should say this precisely so Phase 14's planner doesn't duplicate the classification.

For the SMTP adapter, `lettre` (verified on crates.io: 329k weekly downloads, 2015-origin, `OK` verdict from the package-legitimacy gate) is the standard, and its typed `Message::builder()` API defends against header/CRLF injection **by construction** for recipient (`Mailbox`/`Address` parsing) and free-text headers (`Subject` via RFC 2047 encoded-word encoding) — confirmed by reading the crate's own source, not assumed. The body is a distinct channel (placed after the blank-line header/body separator) and is inert to header injection as long as the adapter never string-concatenates the body literal into the raw header block. Mailpit is the maintained target (MailHog is abandoned since ~2020); use its HTTP API for assertions in the acceptance-gate test, not raw SMTP-transcript parsing.

**Primary recommendation:** Structure the DESIGN doc(s) to (1) change the per-arg loop from first-Block-wins to collect-all-sensitive-taints-then-Block-as-a-set, (2) state precisely that CONTENT-02 changes Step 3's *consequence*, not its classification, (3) require the SMTP adapter use lettre's typed builder exclusively (no string-built headers) pinned to `lettre >= 0.11.22`, and (4) require CONFIRM-03's hash be computed over `ResolvedArg.literal` fields captured at (post-transform) `ValueRecord`-mint time, with an explicit MUST that no transform occurs between mint and Block.

## Architectural Responsibility Map

This project is not a web app; tiers are the project's own locked layers (`CLAUDE.md`).

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Content-sensitivity classification (CONTENT-01/02) | `crates/executor` (TCB) | — | Security-hardcoded decision; never a policy file (`CON-i2-non-bypassable`) |
| Multi-arg Block collection (fixes the B1-reincarnation risk) | `crates/executor` (TCB) | `crates/runtime-core` (shared `ExecutorDecision`/`SinkBlockedAnchor` types) | The decision function and its output shape are both TCB-owned |
| SMTP wire-message construction + CRLF defense (SMTP-05) | `crates/brokerd` adapter (new `sinks::email_smtp` module) | `lettre` (external crate, TCB-adjacent dependency) | Broker is the only process with SMTP secrets/network egress; adapter code is broker-resident, not confined-worker |
| SMTP call execution (SMTP-01) | `crates/brokerd` | — | Confined worker MUST NEVER perform the SMTP call (D-03); default-deny-net (`crates/sandbox`) enforces this negatively |
| Confirm-binding hash computation (CONFIRM-03) | `crates/brokerd` (`confirmation.rs`) | — | Extends the existing `PendingConfirmation`/`ResolvedArg` persisted snapshot; never re-invokes the executor |
| Negative network-deny assertion (SMTP-01's kernel claim) | `crates/sandbox` (existing seccomp filter) | `cli/caprun` worker self-confinement | Already implemented for `AF_INET`/`AF_INET6` `socket()`; this phase's design doc must point SMTP-01's test at this SAME mechanism, not a new one |
| Local capture-SMTP verification (SMTP-03) | External test infra (Mailpit container) | Test harness (`cli/caprun/tests/`) | Not TCB; verification-only, queried via Mailpit's HTTP API |

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `lettre` | `>= 0.11.22` `[VERIFIED: crates.io + github.com/lettre/lettre source]` | Rust SMTP client + typed MIME message builder | Most widely used Rust email crate (329k downloads/week); typed `Mailbox`/`Address`/`HeaderValue` model structurally prevents header injection (see Code Examples) |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| Mailpit (container, not a crate) | latest (`axllent/mailpit` Docker image) `[CITED: mailpit.axllent.org]` | Local capture SMTP server + queryable HTTP API for the acceptance-gate test | SMTP-03's target; run via Colima+Docker on the dev Mac, same as this project's existing Linux-verification pattern |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `lettre` | Hand-rolled raw SMTP client over `TcpStream` | Rejected — reimplements RFC 5321/5322 parsing and dot-stuffing correctly is exactly the kind of "deceptively complex problem" this project's own `CLAUDE.md`/GSD philosophy says not to hand-roll; `lettre`'s typed builder already closes the CRLF-injection class by construction |
| Mailpit | MailHog | Rejected — MailHog is unmaintained since ~2020 (`[CITED: github.com/mailhog/MailHog issue #442 "THIS PROJECT IS DEAD!"]`); Mailpit is its actively-maintained, API-compatible, drop-in successor `[CITED: mailpit.axllent.org, chriswiegman.com]` |

**Installation (for Phase 13, not this phase):**
```bash
cargo add lettre --package brokerd
```

**Version verification:** `crates.io` API confirms `lettre` `max_stable_version = 0.11.22`, last published 2026-05-14 `[VERIFIED: crates.io API queried directly this session]`.

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|--------------|---------|-------------|
| `lettre` | crates.io | ~10 yrs (published 2015-10-21) | 329,096/week | `github.com/lettre/lettre` | OK | Approved |

**Packages removed due to [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none.

`lettre` was cross-checked against `gsd-tools query package-legitimacy check --ecosystem crates lettre` (verdict `OK`, no `postinstall`, not deprecated) AND its own source was read directly from `github.com/lettre/lettre` (the crate's canonical upstream repo) this session — this satisfies the "official documentation or Context7 AND `OK` verdict" bar, so it is tagged `[VERIFIED]` rather than `[ASSUMED]`.

**Security advisories checked (RUSTSEC):**
- `RUSTSEC-2021-0069` (SMTP command injection via double-CRLF dot-stuffing bypass) — affects `<0.9.6` and `0.10.0-alpha.1..<0.10.0-rc.3`; fixed since `>=0.10.0-rc.3`. **Not a concern at `>=0.11.22`.** `[VERIFIED: rustsec.org, fetched directly this session]`
- `RUSTSEC-2026-0141` (TLS hostname verification silently disabled when the `boring-tls` **feature** is enabled; CVSS 9.1; introduced 0.10.1, present through 0.11.21; published 2026-05-14 — the same day 0.11.22 shipped, strongly suggesting 0.11.22 is the fix release). **Only relevant if the `boring-tls` Cargo feature is enabled.** `[CITED: rustsec.org GHSA-4pj9-g833-qx53, via WebSearch — not independently fetched, treat as MEDIUM confidence pending Phase 13's own verification]`. Recommendation: do NOT enable `boring-tls`; the local Mailpit target needs no TLS at all (see Common Pitfalls).

## Architecture Patterns

### System Architecture Diagram

```
  Confined Worker (extractor)                Broker (crates/brokerd) — TCB
  ┌─────────────────────────┐    plan node   ┌──────────────────────────────────────┐
  │ reads hostile doc bytes │ ──ValueId only──▶│ submit_plan_node() [crates/executor] │
  │ mints ValueRecord POST-  │                │  Step 0  schema gate                 │
  │ transform (concat/decode)│                │  loop over ALL args (no early return │
  │ NEVER performs SMTP call │                │    on first Block — see Pitfall 1):  │
  └─────────────────────────┘                │    - resolve ValueId → ValueRecord    │
             ▲                                │    - routing-sensitive + tainted?     │
             │ no network egress               │      → add to blocked-arg set        │
             │ (seccomp denies socket(AF_INET))│    - content-sensitive + tainted?     │
             │ [crates/sandbox]                │      → add to blocked-arg set (NEW:   │
             │                                  │        CONTENT-01/02 changes this    │
             X   direct SMTP attempt FAILS      │        Step-3 consequence, not the    │
                 (SMTP-01 negative assertion)   │        existing classification)       │
                                                │  if blocked-arg set non-empty:        │
                                                │    → BlockedPendingConfirmation        │
                                                │      { anchor: Vec<BlockedArg> }      │
                                                │  else → Step 0.5 draft-only check      │
                                                │  else → Allowed                        │
                                                └──────────────┬────────────────────────┘
                                                                │ persists
                                                                ▼
                                                ┌──────────────────────────────────────┐
                                                │ PendingConfirmation (confirmation.rs)│
                                                │  resolved_args: Vec<ResolvedArg>      │
                                                │  (ALL args, frozen POST-transform)    │
                                                │  + CONFIRM-03 hash over the blocked   │
                                                │    set's literals (NEW field)         │
                                                └──────────────┬────────────────────────┘
                                                                │ caprun confirm <effect_id>
                                                                ▼
                                                ┌──────────────────────────────────────┐
                                                │ SMTP adapter (crates/brokerd/sinks/   │
                                                │  email_smtp.rs, NEW — Phase 13)       │
                                                │  lettre::Message::builder()           │
                                                │   .to(Mailbox) .subject(String)       │
                                                │   .body(String)  ← typed, no string-  │
                                                │   concatenated headers (SMTP-05)      │
                                                │  SmtpTransport → Mailpit :1025         │
                                                └──────────────┬────────────────────────┘
                                                                ▼
                                                       Mailpit container
                                                  (query via HTTP API :8025
                                                   to assert exactly one message)
```

### Recommended Project Structure (for Phases 13-16, informed by this design)

```
crates/brokerd/src/
├── confirmation.rs      # EXTEND: PendingConfirmation gains a combined literal-hash field (CONFIRM-03)
├── sinks/
│   ├── email_smtp.rs    # NEW (Phase 13): lettre-based real adapter, replaces invoke_email_send_stub
│   └── file_create.rs   # existing precedent for a sink adapter module
crates/executor/src/
├── lib.rs                # MODIFY: per-arg loop collects ALL sensitive+tainted args before deciding
├── sink_sensitivity.rs   # NO NEW MATCH ARM NEEDED — is_content_sensitive already covers subject/body/attachment
```

### Pattern 1: Collect-then-Block (fixes the B1-reincarnation risk)

**What:** Change `submit_plan_node`'s per-arg loop from "return `BlockedPendingConfirmation` on the first tainted routing-sensitive arg" to "scan every arg, accumulate every arg that is (routing-sensitive OR content-sensitive) AND tainted into one `Vec<BlockedArg>`, then return one `BlockedPendingConfirmation` carrying the whole set (or `Allowed`/Step-0.5 if the set is empty)."

**When to use:** Any sink with more than one sensitive arg — which, after CONTENT-01, is every live sink (`email.send` has both routing args `to/cc/bcc` and content args `subject/body/attachment`).

**Why this is load-bearing, not stylistic:** Today's `submit_plan_node` (`crates/executor/src/lib.rs:99-139`) returns immediately inside the `for arg in &plan_node.args` loop the instant Step 2's routing check trips. `SinkBlockedAnchor.arg: String` (`crates/runtime-core/src/executor_decision.rs:115`) is singular by construction. If a plan node has BOTH a tainted `to` and a tainted `body`, only ONE of them (whichever appears first in `plan_node.args`) triggers the Block; the other is invisible to the human. Because `confirmation.rs::confirm()` unconditionally re-invokes the sink with the FULL `resolved_args` snapshot (including the arg that was never individually flagged), confirming the shown block silently releases the unshown tainted arg too. This is D-02/D-12(a)'s named risk in a concrete, code-verified form — not a hypothetical.

**Example (shape, not literal code — this crate does not yet have this type):**
```rust
// Source: this session's reading of crates/executor/src/lib.rs + crates/runtime-core/src/executor_decision.rs
// Illustrative target shape for the DESIGN doc, not existing code.
let mut blocked: Vec<BlockedArg> = Vec::new();
for arg in &plan_node.args {
    let record = /* resolve as today, Step 1/1a/1b unchanged */;
    let sensitive = sink_sensitivity::is_routing_sensitive(&plan_node.sink, &arg.name)
        || sink_sensitivity::is_content_sensitive(&plan_node.sink, &arg.name); // CONTENT-01/02
    if sensitive && record.taint.iter().any(|t| t.is_untrusted()) {
        blocked.push(BlockedArg::from_record(&arg.name, &arg.value_id, &record));
    }
}
if !blocked.is_empty() {
    return ExecutorDecision::BlockedPendingConfirmation { anchors: blocked };
}
```

### Pattern 2: Typed SMTP construction — no string-built headers (SMTP-05)

**What:** The adapter constructs the outgoing message exclusively through `lettre::Message::builder()`'s typed setters — never by `format!()`-ing header text and never by embedding a resolved literal into anything other than its own designated builder call.

**When to use:** Every call site in the new `email_smtp.rs` adapter.

**Example:**
```rust
// Source: this session's direct read of github.com/lettre/lettre
// src/message/mod.rs (subject()) and src/address/types.rs (Address::new validation).
use lettre::message::Mailbox;
use lettre::Message;

let to: Mailbox = resolved_to_literal.parse()?;      // rejects CR/LF by allow-list grammar (verified)
let email = Message::builder()
    .to(to)
    .subject(resolved_subject_literal)                // routed through RFC 2047 encoder if it
                                                        // contains bytes 10/13 or non-ASCII — cannot
                                                        // emit raw CR/LF on the wire (verified)
    .body(resolved_body_literal)?;                     // opaque body content, placed after the
                                                        // blank-line header/body separator
```

**Verified mechanics (read directly from `lettre`'s source this session, not assumed):**
- `Address::new` (`src/address/types.rs`) validates the local part via `email_address::EmailAddress::is_valid_local_part`, which is an ALLOW-LIST grammar (`is_atext`/`is_qtext_char`/`is_wsp`) that does not include byte 10 (LF) or byte 13 (CR) in any branch — unquoted, quoted, or escaped-pair local parts all reject raw CR/LF by construction, not by a blocklist check that could miss a case. `[CITED: raw.githubusercontent.com/lettre/lettre/master/src/address/types.rs, read directly this session]`
- `HeaderValueEncoder::allowed_char` (`src/message/header/mod.rs`) explicitly excludes bytes 10 and 13 from its allowed range (`c >= 1 && c <= 9 || c == 11 || c == 12 || c >= 14 && c <= 127`); any header value word containing CR/LF (e.g., a malicious `Subject`) is routed through `email_encoding::headers::rfc2047::encode`, which base64/quoted-printable-encodes the value — raw CR/LF cannot appear on the wire. `[CITED: raw.githubusercontent.com/lettre/lettre/master/src/message/header/mod.rs, read directly this session]`
- `HeaderValue::dangerous_new_pre_encoded`'s own doc comment states it "exposes the encoder to header injection attacks" — confirming, by the crate authors' own words, that the DEFAULT (non-`dangerous_`) path is what closes the injection class. The DESIGN doc should explicitly forbid any adapter use of `dangerous_new_pre_encoded` or hand-built raw header strings.
- Dot-stuffing double-CRLF SMTP-command-injection (RUSTSEC-2021-0069) is fixed since `0.10.0-rc.3`; irrelevant at the recommended `>=0.11.22`.

**Why the body is safe even though it is NOT run through the header encoder:** The body is written after the blank-line separator that terminates the header block (MIME/RFC 5322 structure). A receiving MTA (Mailpit) parses headers up to the first blank line and treats everything after it as opaque body content — a literal `\r\nBcc: attacker@evil.com` string embedded in the body is just body text, never re-parsed as a header, PROVIDED the adapter never concatenates the body literal into the header-construction call chain. This is the concrete answer to D-07: the defense is structural separation (typed builder call boundaries), not string-scrubbing.

### Anti-Patterns to Avoid

- **String-formatting the SMTP envelope/headers ("`format!("To: {}\r\n...")`")**: bypasses every defense above; the DESIGN doc should require a grep-based negative assertion (mirroring `check-invariants.sh`'s style) that no `format!` call in `email_smtp.rs` builds a header line.
- **Treating CONTENT-02 as "add a new match arm"**: `is_content_sensitive` for `email.send`'s `subject`/`body`/`attachment` already exists and already returns `true` (`crates/executor/src/sink_sensitivity.rs:93-98`, since v0). The new work is the Step-3 *consequence* (Block instead of "mark for later, unimplemented"), not a new classification. Writing a "new match arm" task in Phase 14's plan would duplicate existing code.
- **A single-arg Block/anchor shape surviving into CONTENT-01's implementation** — see Pattern 1.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SMTP protocol + MIME message construction | Raw `TcpStream` + hand-written SMTP commands/MIME headers | `lettre::Message::builder()` + `SmtpTransport` | RFC 5321/5322 correctness (dot-stuffing, header folding, encoded-words) is exactly the "deceptively complex" class this project avoids hand-rolling elsewhere (executor, taint model) |
| CRLF/header-injection sanitization | A custom regex/string-scrubber that strips `\r`/`\n` from recipient or subject before handing to lettre | Rely on `lettre`'s typed `Mailbox`/`Address`/`HeaderValue` parse-time rejection (allow-list grammar) | A hand-rolled sanitizer is a second, weaker, ad hoc check duplicating what the typed builder already guarantees by construction — and duplication is itself a bug source (one path might get missed) |
| Capture-SMTP verification | Parsing raw SMTP session logs or `.eml` files from disk | Mailpit's HTTP API (`GET /api/v1/messages`, `GET /api/v1/message/{id}`) `[ASSUMED — exact paths per training knowledge cross-referenced with WebSearch snippets, not independently fetched this session; Phase 13 must confirm against the live container's `/api/v1/` interactive docs before writing the test]` | Purpose-built, stable JSON API vs. brittle log/file scraping |
| Confirm-binding hash function | A new hashing scheme for CONFIRM-03 | SHA-256, matching the existing `literal_sha256` pattern in `SinkBlockedAnchor` (`crates/executor/src/lib.rs:112-116`) | Consistency with the established tamper-evidence pattern; no reason to introduce a second hash primitive |

**Key insight:** Every "don't hand-roll" item in this phase's domain already has an existing, verified-by-construction solution either in the recommended external crate (`lettre`) or in this repo's own prior art (SHA-256 digest pattern, `PendingConfirmation` snapshot). The design work is composition and precedence, not new primitives.

## Common Pitfalls

### Pitfall 1: B1 reincarnated for the body arg (D-02, D-12a) — the central risk of this phase

**What goes wrong:** A tainted recipient (`to`) and a tainted body both occupy sensitive args on the same `PlanNode`. The per-arg loop's first-match-wins early return (current code) Blocks on `to` only. The human confirms the `to` block. `confirmation.rs::confirm()` re-invokes the sink using the FULL `resolved_args` snapshot — which includes the body's literal, never individually shown or confirmed. The tainted body ships.

**Why it happens:** `SinkBlockedAnchor`/`ExecutorDecision::BlockedPendingConfirmation` are shaped around exactly one blocked arg (verified: `crates/runtime-core/src/executor_decision.rs:108-129`), and the executor returns on first match (verified: `crates/executor/src/lib.rs:99-139`).

**How to avoid:** See Pattern 1 (collect-then-Block). The DESIGN doc must state this as a MUST, not an implementation detail left to Phase 14.

**Warning signs:** Any DESIGN doc draft that discusses CONTENT-01 purely in terms of "add a Block for tainted body" without addressing what happens when recipient AND body are BOTH tainted in the same plan node.

### Pitfall 2: CONFIRM-03's hash computed over pre-transformation bytes (D-12b)

**What goes wrong:** Phase 15's EXTRACT-03 requires the extractor to TRANSFORM tainted values (concatenate two doc fields, base64-decode) before they reach the sink. If the `ValueRecord` is minted BEFORE the transform (i.e., the worker mints a `ValueRecord` for the raw untransformed fragment, and the concatenation/decoding happens later, e.g., in the planner or adapter, operating on the already-resolved literal), then `ResolvedArg.literal` — and therefore any CONFIRM-03 hash computed from it — would not match the bytes actually sent. The human would be confirming (and the hash would attest to) different bytes than what the adapter transmits.

**Why it happens:** `ValueRecord.literal` is "the exact string that would be passed to the sink" ONLY if minting happens after every transform. Nothing in the current codebase prevents a future extractor implementation from minting early and transforming late.

**How to avoid:** The DESIGN doc must state, as a MUST: the extractor (confined worker) mints the `ValueRecord` (via `mint_from_read` or its Phase-15 successor) ONLY AFTER applying any transformation to the raw read bytes; there is NO transformation step permitted between `ValueRecord` mint and executor Block, and none between Block (frozen into `ResolvedArg.literal`) and adapter invocation. CONFIRM-03's hash is computed over `ResolvedArg.literal` fields, which are then guaranteed-by-this-rule to equal the sink-bound bytes.

**Warning signs:** Any Phase 15 design or code that resolves a `ValueId` to its literal and then performs a string operation (concatenation, decode) on the result before using it as a sink arg, rather than minting a fresh `ValueRecord` for the transformed value with inherited taint.

### Pitfall 3: Tainted literal reaching a header via string-built adapter code (D-12c)

**What goes wrong:** Even with `lettre`'s typed builder available, a future adapter implementation could bypass its protections by hand-formatting header text (e.g., building a raw MIME string for logging, or using `dangerous_new_pre_encoded` for a "simpler" implementation) — reintroducing exactly the injection class `lettre` closes by construction.

**Why it happens:** `lettre` offers an escape hatch (`dangerous_new_pre_encoded`) whose own doc comment admits it exposes injection risk; a future contributor unaware of D-07/SMTP-05 could reach for it.

**How to avoid:** DESIGN doc states explicitly: adapter code MUST use only the safe/default `Message::builder()` setter methods (`.to()`, `.cc()`, `.bcc()`, `.subject()`, `.body()`); MUST NOT call `dangerous_new_pre_encoded` or any raw/string-formatted header path. Recommend a grep-based negative-assertion test (mirroring `check-invariants.sh`'s existing style) asserting the token `dangerous_new_pre_encoded` never appears in `crates/brokerd/src/sinks/email_smtp.rs`.

**Warning signs:** Any code review of the Phase 13 adapter that finds `format!` building a `To:`/`Subject:`/`Bcc:` line, or any use of `dangerous_new_pre_encoded`.

### Pitfall 4: Enabling `lettre`'s `boring-tls` feature

**What goes wrong:** `RUSTSEC-2026-0141` — an inverted-boolean bug silently disables TLS hostname verification when the `boring-tls` Cargo feature is compiled in, for versions `0.10.1..=0.11.21`. `[CITED: rustsec.org, via WebSearch this session — not independently fetched]`

**Why it happens:** Feature-gated code paths are easy to enable without realizing the security implication, especially when copying a Cargo.toml snippet from an unrelated example.

**How to avoid:** Do not enable `boring-tls`. The local Mailpit target on `localhost:1025` needs no TLS at all — default (rustls or none) is both simpler and unaffected by this advisory.

### Pitfall 5: Testing SMTP-01's negative assertion against a mechanism the design doc didn't name

**What goes wrong:** The DESIGN doc describes "confined worker's direct SMTP connection attempt FAILS" abstractly, and a later phase writes a NEW confinement mechanism to prove it, duplicating the existing one.

**Why it happens:** Without reading `crates/sandbox`, it's easy to assume network denial needs new code.

**How to avoid:** Network denial is ALREADY implemented: `crates/sandbox/src/seccomp.rs::apply_worker_filter()` installs a seccomp-bpf filter denying `socket(AF_INET, ...)` and `socket(AF_INET6, ...)` with `SeccompAction::Errno(EPERM)`. The existing test pattern (`crates/sandbox/tests/confinement_integration.rs::negative_net`, `crates/sandbox/src/bin/confine-probe.rs::probe_net`) already proves `socket(AF_INET, SOCK_STREAM, 0)` returns EPERM under confinement. The DESIGN doc should point SMTP-01's negative assertion at THIS exact mechanism — e.g., a new integration test that spawns a worker-like confined process attempting an actual SMTP `connect()` (not just `socket()`) to the Mailpit host:port, asserting `EPERM`/`ECONNREFUSED`-via-blocked-socket, reusing `confine-probe`'s pattern rather than inventing a new confinement primitive. Landlock does not restrict socket creation (confirmed in `probe_net`'s own doc comment); only seccomp can produce this EPERM.

## Code Examples

### Existing verified pattern: SHA-256 digest for a blocked literal (reuse for CONFIRM-03)

```rust
// Source: crates/executor/src/lib.rs:112-116 (this repo, read directly this session)
let literal_sha256 = {
    let mut hasher = Sha256::new();
    hasher.update(record.literal.as_bytes());
    hex::encode(hasher.finalize())
};
```

### Existing verified pattern: content-sensitivity classification (already implemented — CONTENT-02's "one match arm")

```rust
// Source: crates/executor/src/sink_sensitivity.rs:93-98 (this repo, read directly this session)
pub fn is_content_sensitive(sink: &SinkId, arg_name: &str) -> bool {
    match sink.0.as_str() {
        "email.send" => EMAIL_SEND_CONTENT_SENSITIVE.contains(&arg_name), // ["subject", "body", "attachment"]
        _ => false,
    }
}
```
This ALREADY satisfies D-01/CONTENT-02's "single hardcoded match arm" requirement. What CONTENT-01 changes is `submit_plan_node`'s Step 3, which today is a no-op comment ("do NOT Block in v0 — Tier-4 verbatim review deferred").

### lettre typed builder (SMTP-05 pattern)

```rust
// Source: read directly from github.com/lettre/lettre this session (src/message/mod.rs, src/address/types.rs)
use lettre::{Message, SmtpTransport, Transport};
use lettre::message::Mailbox;

let email = Message::builder()
    .to(resolved_to_literal.parse::<Mailbox>()?)   // CR/LF rejected by allow-list grammar
    .subject(resolved_subject_literal)              // CR/LF-containing values RFC2047-encoded
    .body(resolved_body_literal)?;                  // opaque body content

let mailer = SmtpTransport::builder_dangerous("localhost") // "dangerous" here means "no TLS",
    .port(1025)                                            // appropriate for a local plaintext
    .build();                                               // Mailpit target, NOT a header-injection risk
mailer.send(&email)?;
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| MailHog as the standard local capture SMTP tool | Mailpit | MailHog effectively abandoned since ~2020 (`[CITED: github.com/mailhog/MailHog#442]`); Mailpit is the actively maintained, API-compatible successor adopted by Laravel Sail, DDEV, etc. | Use Mailpit for SMTP-03's acceptance-gate target, not MailHog, despite REQUIREMENTS.md/ROADMAP.md naming both interchangeably |
| `lettre` <0.10 dot-stuffing-vulnerable SMTP transport | `lettre` >=0.10.0-rc.3 (current 0.11.22) | RUSTSEC-2021-0069 fix | Pin `>=0.11.22` in Cargo.toml to also pick up the RUSTSEC-2026-0141 TLS fix (relevant only if `boring-tls` is ever enabled) |

**Deprecated/outdated:**
- MailHog: unmaintained; do not build new integration test tooling against it even though it's named in REQUIREMENTS.md/ROADMAP.md as an "either/or" — Mailpit is the correct target.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Mailpit's exact HTTP API endpoint paths (`GET /api/v1/messages`, `GET /api/v1/message/{id}`, `GET /api/v1/search`) | Don't Hand-Roll table | Low — Phase 13's implementer will hit Mailpit's own `/api/v1/` interactive docs immediately when writing the test; wrong path names here don't block design-doc writing, only need correction before Phase 13 code |
| A2 | `RUSTSEC-2026-0141`'s exact affected-version range and fix-release correlation with 0.11.22 | Package Legitimacy Audit / Pitfall 4 | Low-Medium — sourced from WebSearch summary of rustsec.org, not independently re-fetched via a working WebFetch call this session; if wrong, the `boring-tls` avoidance advice is still safe regardless (avoiding an unused feature has no downside) |

**If this table is empty:** N/A — see above; both assumptions are low-risk and self-correcting at the next phase.

## Open Questions

1. **Should CONTENT-01's Block for a tainted content-sensitive arg be identical in shape to the existing routing-sensitive Block (same `ExecutorDecision::BlockedPendingConfirmation` variant), or does D-08's "hash of resolved recipient+body" imply the DESIGN doc should also define a NEW always-both-shown combined-Block shape (Pattern 1) as a hard requirement rather than a recommendation?**
   - What we know: current code structurally supports one arg per Block; D-08's phrasing ("hash of resolved recipient+body literals") reads naturally as a single combined confirmation.
   - What's unclear: whether the DESIGN doc's authors (Phase 12) should mandate the multi-arg collection as the ONE way to satisfy D-02+D-08 together, or leave open a design where confirm is per-arg but CONFIRM-03's hash still spans multiple confirmed effect_ids.
   - Recommendation: the DESIGN doc should mandate the single-collected-Block-set design (Pattern 1) — it's the only shape that makes D-02's "both surface as Blocked... and can be confirmed/denied through the single-shot mechanism" and D-08's "hash of resolved recipient+body" simultaneously true without a second confirm round-trip.

2. **Does the adversarial reviewer (D-11/D-12) need to independently re-verify the `is_content_sensitive` "already exists" finding, or can the DESIGN doc simply cite this research?**
   - What we know: verified directly in this session by reading `crates/executor/src/sink_sensitivity.rs`.
   - What's unclear: whether the fresh-context reviewer arranged per D-11 should re-run the same grep/read independently (recommended, since D-11 requires genuine adversarial re-verification, not trust in this research doc).
   - Recommendation: the DESIGN doc should state the finding with a pointer to the exact file/line so the reviewer can re-verify in seconds rather than re-discovering it.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Docker / Colima | Linux-gated tests (SMTP-01 negative assertion, Mailpit integration) | ✓ (per project memory: "Colima installed", used throughout v1.0-v1.2) | — | none needed |
| Mailpit container | SMTP-03 acceptance-gate target (Phase 13, not this phase) | Not yet pulled/verified this session | — | `docker run -d -p 8025:8025 -p 1025:1025 axllent/mailpit` per official docs `[CITED: mailpit.axllent.org]` |
| `lettre` crate | SMTP-05 adapter (Phase 13, not this phase) | Not yet added to any `Cargo.toml` | `0.11.22` on crates.io | none needed |

**Missing dependencies with no fallback:** none — this phase itself has no external dependencies (documentation only). The above table is forward-looking for Phase 13.

## Validation Architecture

This phase produces DESIGN documentation only — no code, no test files. The "validation" for this phase is the DESIGN-GATE-RECORD adversarial-review process itself, not a unit/integration test suite.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | N/A — no code in this phase |
| Config file | N/A |
| Quick run command | N/A |
| Full suite command | N/A |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DESIGN-01 | A reviewed DESIGN doc exists, adversarially reviewed, gate record recorded APPROVED before Phases 13-16 write code | checkpoint (human/AI-adversarial review, per D-11) | grep-based completeness checklist mirroring `planning-docs/DESIGN-GATE-RECORD-v1.2.md`'s pattern (see below) | ❌ — Wave 0 of this phase's plan must author the checklist + gate record template |

### Sampling Rate
- **Per task commit:** N/A (doc-only phase)
- **Per wave merge:** re-run the grep-completeness checklist against the current doc draft
- **Phase gate:** DESIGN-GATE-RECORD.md shows `Decision: APPROVED` and `Gate status: UNBLOCKED`, per D-11/D-13, before this phase is marked complete

### Wave 0 Gaps
- [ ] `planning-docs/DESIGN-content-adapter-mediation.md` (or split docs, per Claude's Discretion) — does not yet exist
- [ ] `planning-docs/DESIGN-GATE-RECORD-v1.3.md` (or similarly named) — must be authored following the `DESIGN-GATE-RECORD-v1.2.md` structure (Documents Under Review + sha256 table, Checklist mapped to CONTENT-01/02, SMTP-01/02/03/05, CONFIRM-03, "How to Verify" steps, Decision/Gate-status fields)
- [ ] The three D-12 attack vectors (a/b/c) must each have a dedicated, named section in the gate record's "How to Verify" steps — mirroring how `DESIGN-REVIEW-v1.2-round1.md`'s B1 finding was traced through actual code line numbers, not asserted abstractly

*(No traditional test-framework gaps — this phase's "tests" are the checklist grep assertions against the doc's own text, exactly as `DESIGN-GATE-RECORD-v1.2.md` did.)*

## Security Domain

### Applicable ASVS Categories

This project is a local CLI/security-runtime tool, not a web app with user auth/sessions — most V2-V4 ASVS categories don't apply. The relevant category is input validation against injection.

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | No user-facing auth surface in this project |
| V3 Session Management | no | `SessionStatus` here is an internal trust-state machine (I0/I1), not a web session — already covered by v1.2's design docs, not this phase's scope |
| V4 Access Control | no | Covered by the executor's sink-callability gate (`DESIGN-plan-executor.md`), not new in this phase |
| V5 Input Validation | yes | SMTP header/CRLF injection (SMTP-05) — `lettre`'s typed `Mailbox`/`HeaderValue` allow-list parsing (verified this session); this is the primary ASVS surface this phase's design doc must close |
| V6 Cryptography | yes (narrow) | CONFIRM-03's literal-binding hash — SHA-256, matching the existing `literal_sha256` pattern; never hand-roll a new digest scheme |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| SMTP header/CRLF injection via a tainted body or subject smuggling a `Bcc:`/extra recipient | Tampering / Elevation of Privilege (routing redirect) | `lettre`'s typed builder (allow-list address grammar + RFC2047 header encoding); structural header/body separation |
| Confirm-then-smuggle: confirming ONE blocked arg silently releases OTHER unconfirmed tainted args in the same plan node | Tampering (this phase's central finding — Pitfall 1) | Collect-then-Block (Pattern 1); CONFIRM-03 hash spans the full blocked set |
| TOCTOU on the confirm-binding hash (confirmed bytes ≠ sent bytes, via EXTRACT-03 transforms) | Tampering | Mint `ValueRecord` only AFTER any transform; no transform between mint and Block (Pitfall 2) |

## Sources

### Primary (HIGH confidence)
- `crates/executor/src/lib.rs`, `crates/executor/src/sink_sensitivity.rs`, `crates/executor/src/sink_schema.rs`, `crates/executor/src/value_store.rs` — read directly this session
- `crates/runtime-core/src/executor_decision.rs`, `crates/runtime-core/src/plan_node.rs` (partial, via grep) — read directly this session
- `crates/brokerd/src/confirmation.rs` — read directly this session
- `crates/sandbox/src/seccomp.rs`, `crates/sandbox/src/lib.rs`, `crates/sandbox/tests/confinement_integration.rs`, `crates/sandbox/src/bin/confine-probe.rs` (partial) — read directly this session
- `planning-docs/DESIGN-taint-model.md`, `planning-docs/DESIGN-plan-executor.md`, `planning-docs/DESIGN-session-trust-state.md`, `planning-docs/DESIGN-confirmation-release.md`, `planning-docs/DESIGN-REVIEW-v1.2-round1.md`, `planning-docs/DESIGN-GATE-RECORD-v1.2.md` — read directly this session
- `.planning/phases/12-content-adapter-confirm-binding-design-gate/12-CONTEXT.md`, `.planning/REQUIREMENTS.md`, `.planning/STATE.md`, `.planning/PROJECT.md`, `.planning/ROADMAP.md` — read directly this session
- `github.com/lettre/lettre` source (`src/message/mod.rs`, `src/message/header/mod.rs`, `src/address/types.rs`) — fetched and read directly this session via raw.githubusercontent.com
- `github.com/johnstonskj/rust-email_address` source (`src/lib.rs`) — fetched and read directly this session
- `rustsec.org/advisories/RUSTSEC-2021-0069.html` — fetched directly this session
- `crates.io` API (`GET /api/v1/crates/lettre`) — queried directly this session; `gsd-tools query package-legitimacy check` — run directly this session

### Secondary (MEDIUM confidence)
- Mailpit maintenance status and MailHog abandonment — `mailpit.axllent.org`, `chriswiegman.com`, `github.com/mailhog/MailHog#442` (WebSearch, official-docs-adjacent)
- `RUSTSEC-2026-0141` details — WebSearch summary of `rustsec.org`, not independently re-fetched

### Tertiary (LOW confidence)
- Exact Mailpit HTTP API endpoint paths (`/api/v1/messages`, `/api/v1/message/{id}`, `/api/v1/search`) — WebSearch snippets only; flagged in Assumptions Log (A1) for Phase 13 to confirm against the live container

## Metadata

**Confidence breakdown:**
- Standard stack (lettre, Mailpit): HIGH — lettre verified via package-legitimacy gate + direct source read; Mailpit vs MailHog verified via multiple independent secondary sources
- Architecture (collect-then-Block, content-sensitivity-already-exists, CRLF-by-construction): HIGH — all grounded in direct reads of this repo's own code and lettre's own upstream source, not inference
- Pitfalls: HIGH for Pitfalls 1/2/3/5 (derived from direct code reads); MEDIUM for Pitfall 4 (RUSTSEC advisory details via WebSearch, not re-fetched)

**Research date:** 2026-07-07
**Valid until:** 30 days (stable domain — Rust crate ecosystem and this project's own architecture change slowly; re-verify `lettre` version and RUSTSEC status before Phase 13 if more than a few weeks pass)
