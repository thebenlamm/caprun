//! sinks/git_push — the pure, host-portable protocol substrate for the
//! broker-performed `git.push` smart-HTTP transfer (GIT-02, DESIGN-v1.9-egress
//! §1.1/§1.3, RESEARCH §2/§3/§5).
//!
//! # What lives here (Plan 44-02)
//!
//! This module is the PURE half of the broker-driven push: pkt-line
//! encode/decode, the ref-advertisement parser, the report-status parser, the
//! `validate_git_refspec` value-gate, and the receive-pack command-list builder
//! whose construction makes `--force`/deletion UNREACHABLE. Every function here
//! is pure byte manipulation — no socket, no `git` binary, no async — so it is
//! fully unit-tested on the macOS host (CLAUDE.md: no cfg-gating needed for pure
//! code). Plan 44-03 ADDED, in this module: the confined `git pack-objects` PACK
//! bytes (via the WG-2 binary launcher), the broker-env credential + distinct
//! `GIT_PUSH_HOST_ALLOWLIST` + opaque scrubbed two-phase audit, and the
//! `invoke_git_push_from_resolved` two-request driver (which CONSUMES the 44-02
//! substrate through the WG-1 frozen-IP client).
//!
//! The driver's Linux socket/`git` legs are `#[cfg(target_os = "linux")]`; on the
//! macOS host they stub out, so the substrate parsers/command-list/pack helpers
//! are reachable there only from this module's own unit tests — a non-test macOS
//! `cargo build` sees them as unused, hence the module-scoped `allow(dead_code)`.
//! It narrows once Plan 44-04's confirm-release Step-7 dispatch arm consumes
//! `invoke_git_push_from_resolved` on the (Linux) live path.
//!
//! # Structural `--force`/deletion denial (DESIGN §1.3, RESEARCH §5)
//!
//! Two defense-in-depth value-level layers, BOTH here:
//!   1. `validate_git_refspec` rejects a leading `+` (force), an empty `<src>`
//!      (`:dst` deletion), and any `--force`/`--force-with-lease`-shaped token —
//!      the exact pattern of `http_request.rs::validate_write_method`.
//!   2. `build_command_list` REFUSES to construct a receive-pack line whose
//!      `<new-oid>` is the zero-oid (delete), for ANY input, and emits a fixed
//!      capability set carrying NO force capability — so a force update / a
//!      deletion is not expressible by any code path, unreachable even via a
//!      human confirm (a human confirms a specific push, not a license to
//!      rewrite history).
//!
//! The `<old-oid>` embedded in a command line is ALWAYS a caller-supplied
//! parameter sourced from the frozen info/refs advertisement (Plan 44-03), NEVER
//! read from the untrusted local repo (WG-6/T-44-07).
//!
//! # NO mint / NO audit here (Gate 3)
//!
//! Like `http_request.rs`, this module performs NO `ValueStore::mint`, appends
//! NO audit `Event`, and never touches session status — that keeps it out of
//! `check-invariants.sh` Gate 3's mint-site restriction. Plan 44-03 owns the
//! opaque `git_push_succeeded`/`_failed` audit surface.
#![allow(dead_code)]

use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use runtime_core::Event;
use rusqlite::Connection;
use std::collections::BTreeMap;
use uuid::Uuid;

use adapter_fs::workspace::WorkspaceRoot;

use crate::audit::append_event;
use crate::confirmation::ResolvedArg;
use crate::sinks::http_request;

// The response-body cap primitives (RESEARCH A6) are consumed only by the
// Linux-gated transfer helpers; the macOS stub does not touch a socket.
#[cfg(target_os = "linux")]
use crate::sinks::http_request::{build_pinned_client, check_body_cap, resolve_and_vet, MAX_RESPONSE_BODY_BYTES};

// The confined-launcher machinery (Pattern B) is consumed only by the Linux-gated
// pack-gen + rev-parse children; the macOS stubs take the same signatures but do
// not spawn, so these imports are Linux-only (mirrors the platform split).
#[cfg(target_os = "linux")]
use crate::sinks::process_exec::{resolve_launcher_path, run_launcher, run_launcher_capture_bytes};

/// The 4-byte flush-pkt marker terminating a pkt-line stream / section.
const FLUSH_PKT: &[u8] = b"0000";

/// Maximum pkt-line payload: `0xffff` (the largest length a 4-hex prefix can
/// encode) minus the 4-byte prefix itself. Git's own pkt-line limit. Our
/// command lines are tiny (< 200 bytes); this only documents the bound.
const MAX_PKT_PAYLOAD: usize = 0xffff - 4;

/// The zero object id in SHA-1 (40 hex) and SHA-256 (64 hex) widths. A push
/// whose `<new-oid>` is the zero-oid is a DELETE (structurally refused, §1.3); a
/// push whose `<old-oid>` is the zero-oid (ref not advertised) is a CREATE
/// (allowed, WG-6).
const ZERO_OID_SHA1: &str = "0000000000000000000000000000000000000000";
const ZERO_OID_SHA256: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// A decoded pkt-line: either a data payload or the flush marker.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Pkt {
    Data(Vec<u8>),
    Flush,
}

// ---- ENCODE ----

/// Encode one pkt-line: the 4-hex big-endian length (`payload.len() + 4`, over
/// the payload PLUS the 4-byte prefix) followed by the payload. Pure.
fn pkt_line(payload: &[u8]) -> Vec<u8> {
    // Our command lines are far under the limit; a debug_assert documents the
    // invariant without forcing a `Result` on every tiny encode.
    debug_assert!(
        payload.len() <= MAX_PKT_PAYLOAD,
        "pkt-line payload exceeds the 65531-byte limit"
    );
    let len = payload.len() + 4;
    let mut out = format!("{len:04x}").into_bytes();
    out.extend_from_slice(payload);
    out
}

/// The flush-pkt (`0000`) that terminates a pkt-line section.
fn flush_pkt() -> &'static [u8] {
    FLUSH_PKT
}

// ---- DECODE ----

/// Parse a 4-byte ASCII-hex length header into its numeric value. Fail-closed on
/// a non-4-byte / non-utf8 / non-hex header.
fn parse_hex4(b: &[u8]) -> Result<usize> {
    if b.len() != 4 {
        bail!("pkt-line: length header must be exactly 4 bytes");
    }
    let s = std::str::from_utf8(b).map_err(|_| anyhow!("pkt-line: non-utf8 length header"))?;
    usize::from_str_radix(s, 16).map_err(|_| anyhow!("pkt-line: non-hex length header {s:?}"))
}

/// Decode ONE pkt-line from the front of `buf`, advancing it past the consumed
/// bytes. Returns:
///   - `Ok(None)` when `buf` is empty (clean end of stream),
///   - `Ok(Some(Pkt::Flush))` for a `0000` flush,
///   - `Ok(Some(Pkt::Data(_)))` for the `length-4` payload bytes,
///   - `Err` (fail-closed) for a truncated/malformed length or a truncated
///     payload — never a partial/silent read.
fn read_pkt(buf: &mut &[u8]) -> Result<Option<Pkt>> {
    if buf.is_empty() {
        return Ok(None);
    }
    if buf.len() < 4 {
        bail!("pkt-line: truncated length header (have {} bytes)", buf.len());
    }
    let len = parse_hex4(&buf[0..4])?;
    if len == 0 {
        *buf = &buf[4..];
        return Ok(Some(Pkt::Flush));
    }
    // 0001/0002/0003 are protocol-v2 special pkts we do not use; a non-flush
    // length under 4 is malformed for the receive-pack v0 subset — fail closed.
    if len < 4 {
        bail!("pkt-line: invalid non-flush length {len} (< 4)");
    }
    if buf.len() < len {
        bail!(
            "pkt-line: truncated payload (length header says {len}, only {} bytes remain)",
            buf.len()
        );
    }
    let payload = buf[4..len].to_vec();
    *buf = &buf[len..];
    Ok(Some(Pkt::Data(payload)))
}

// ---- shared oid / line helpers ----

/// True iff `oid` is the all-zero object id (a valid SHA-1 or SHA-256 width of
/// all `0` chars). A push with a zero `<new-oid>` is a delete (refused, §1.3).
fn is_zero_oid(oid: &str) -> bool {
    oid == ZERO_OID_SHA1 || oid == ZERO_OID_SHA256
}

/// Fail-closed object-id shape check: exactly 40 (SHA-1) or 64 (SHA-256) ASCII
/// hex digits. Rejects garbage before it can reach a command line. The all-zero
/// oid passes (a legitimate create's `<old-oid>`).
fn validate_oid(oid: &str) -> Result<()> {
    let ok = (oid.len() == 40 || oid.len() == 64) && oid.bytes().all(|b| b.is_ascii_hexdigit());
    if ok {
        Ok(())
    } else {
        bail!("git.push: malformed object id {oid:?} (want 40 or 64 hex digits)");
    }
}

/// Strip a single trailing LF from a pkt-line payload (advertisement + report
/// lines conventionally end with `\n`).
fn strip_trailing_lf(line: &[u8]) -> &[u8] {
    match line.last() {
        Some(&b'\n') => &line[..line.len() - 1],
        _ => line,
    }
}

/// Split a ref-advertisement line into its `<oid> SP <refname>` part and the
/// capability list. Only the FIRST ref line carries capabilities after a NUL;
/// later lines have no NUL and yield an empty capability list.
fn split_ref_and_caps(line: &[u8]) -> (&[u8], Vec<String>) {
    if let Some(nul) = line.iter().position(|&b| b == 0) {
        let refpart = &line[..nul];
        let caps = String::from_utf8_lossy(&line[nul + 1..])
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        (refpart, caps)
    } else {
        (line, Vec::new())
    }
}

/// Parse one `<oid> SP <refname>` ref part into `(oid, refname)`. Fail-closed on
/// a missing field or a malformed oid.
fn parse_ref_line(refpart: &[u8]) -> Result<(String, String)> {
    let s =
        std::str::from_utf8(refpart).map_err(|_| anyhow!("advertisement: non-utf8 ref line"))?;
    let mut it = s.splitn(2, ' ');
    let oid = it.next().unwrap_or("");
    let refname = it.next().unwrap_or("");
    if oid.is_empty() || refname.is_empty() {
        bail!("advertisement: malformed ref line {s:?}");
    }
    validate_oid(oid)?;
    Ok((oid.to_string(), refname.to_string()))
}

// ---- ADVERTISEMENT parse ----

/// The parsed `git-receive-pack` ref advertisement: the capability list from the
/// first ref line and the advertised `refname -> oid` map.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Advertisement {
    caps: Vec<String>,
    refs: BTreeMap<String, String>,
}

impl Advertisement {
    /// The advertised old-oid for `refname`, or `None` if the ref is NOT
    /// advertised — the CREATE case (WG-6): the caller uses the zero-oid as the
    /// `<old-oid>`, and `build_command_list` allows it (the refusal keys on
    /// `<new-oid>`, never on the old-oid).
    fn old_oid_for(&self, refname: &str) -> Option<&str> {
        self.refs.get(refname).map(String::as_str)
    }
}

/// Parse a smart-HTTP `git-receive-pack` info/refs advertisement. Skips the
/// leading `# service=git-receive-pack` announcement pkt + its flush (when
/// present), reads the capability list from the FIRST ref line (split on NUL),
/// collects `refname -> oid`, and stops at the terminating flush. Fail-closed on
/// a malformed line or an unterminated stream.
fn parse_advertisement(body: &[u8]) -> Result<Advertisement> {
    let mut buf: &[u8] = body;
    let mut caps: Vec<String> = Vec::new();
    let mut refs: BTreeMap<String, String> = BTreeMap::new();
    let mut first_ref = true;

    let mut pending = read_pkt(&mut buf)?;

    // Optional smart-HTTP service announcement: "# service=..." then a flush.
    if let Some(Pkt::Data(ref d)) = pending {
        if d.starts_with(b"# service=") {
            match read_pkt(&mut buf)? {
                Some(Pkt::Flush) => {}
                _ => bail!("advertisement: service announcement not followed by a flush"),
            }
            pending = read_pkt(&mut buf)?;
        }
    }

    loop {
        match pending {
            None => bail!("advertisement: unterminated (no flush before end of stream)"),
            Some(Pkt::Flush) => break,
            Some(Pkt::Data(line)) => {
                let line = strip_trailing_lf(&line);
                let (refpart, line_caps) = split_ref_and_caps(line);
                if first_ref {
                    caps = line_caps;
                    first_ref = false;
                }
                let (oid, refname) = parse_ref_line(refpart)?;
                refs.insert(refname, oid);
            }
        }
        pending = read_pkt(&mut buf)?;
    }

    Ok(Advertisement { caps, refs })
}

// ---- REPORT-STATUS parse ----

/// Parse a `git-receive-pack` report-status response. Requires a clean
/// `unpack ok` AND at least one per-ref `ok <ref>`; ANY `unpack <err>`,
/// `ng <ref> <reason>`, or unrecognized status line is a fail-closed push
/// failure. Per RESEARCH §3 we do NOT advertise `side-band-64k`, so report-status
/// arrives on the main band (no band demux). T-44-08: a hidden `ng` cannot be
/// silently accepted.
fn parse_report_status(body: &[u8]) -> Result<()> {
    let mut buf: &[u8] = body;
    let mut unpack_ok = false;
    let mut ref_ok_count = 0usize;

    loop {
        match read_pkt(&mut buf)? {
            None | Some(Pkt::Flush) => break,
            Some(Pkt::Data(line)) => {
                let line = strip_trailing_lf(&line);
                let s = std::str::from_utf8(line)
                    .map_err(|_| anyhow!("report-status: non-utf8 status line"))?;
                if let Some(rest) = s.strip_prefix("unpack ") {
                    if rest == "ok" {
                        unpack_ok = true;
                    } else {
                        bail!("git.push: remote unpack failed: {rest}");
                    }
                } else if s.strip_prefix("ok ").is_some() {
                    ref_ok_count += 1;
                } else if let Some(rest) = s.strip_prefix("ng ") {
                    bail!("git.push: remote rejected ref update: {rest}");
                } else {
                    bail!("git.push: unrecognized report-status line {s:?}");
                }
            }
        }
    }

    if !unpack_ok {
        bail!("git.push: report-status missing a clean 'unpack ok'");
    }
    if ref_ok_count == 0 {
        bail!("git.push: report-status contained no per-ref 'ok' status");
    }
    Ok(())
}

// ---- refspec value-gate (structural-denial layer 1, RESEARCH §5) ----

/// Value-gate on a push refspec, mirroring `http_request.rs::validate_write_method`
/// (the executor Step-0 name-set schema gate cannot see a refspec VALUE, so the
/// force/deletion refusal lives here). Fail-closed rejects:
///   - a leading `+` (a force / non-fast-forward update),
///   - any `--force` / `--force-with-lease`-shaped or other `--flag` token (a
///     refspec must never look like a CLI flag),
///   - an empty `<src>` in `<src>:<dst>` (`:dst`), i.e. a ref DELETION, and a
///     `<dst>` that is empty or carries a further `:`.
/// Returns `Ok(())` for a plain `<src>:<dst>` or a bare `<ref>` non-force refspec.
///
/// Called by BOTH the confirm-precheck (Plan 44-03 Step 4.8d) and the transfer
/// path so the two cannot drift (the P34 lesson).
pub(crate) fn validate_git_refspec(refspec: &str) -> Result<()> {
    if refspec.is_empty() {
        bail!("git.push: empty refspec is refused");
    }
    // Force-push prefix.
    if refspec.starts_with('+') {
        bail!("git.push: force-push refspec (leading '+') is refused");
    }
    // Any CLI-flag / --force-shaped token. A legitimate refspec never begins
    // with '--', and a `--force`/`--force-with-lease` substring is never valid
    // inside one — reject both, case-insensitively.
    let lower = refspec.to_ascii_lowercase();
    if refspec.starts_with("--") || lower.contains("--force") {
        bail!("git.push: --force/--force-with-lease/flag-shaped refspec token is refused");
    }
    // Deletion + malformed <src>:<dst>.
    if let Some((src, dst)) = refspec.split_once(':') {
        if src.is_empty() {
            bail!("git.push: deletion refspec (empty <src> / bare ':dst') is refused");
        }
        if dst.is_empty() {
            bail!("git.push: refspec has an empty <dst>");
        }
        if dst.contains(':') {
            bail!("git.push: malformed refspec (more than one ':')");
        }
    }
    Ok(())
}

// ---- receive-pack command-list (structural-denial layer 2, RESEARCH §5) ----

/// The fixed capability set on the FIRST (here, only) receive-pack command line.
/// Deliberately carries `report-status` + `agent` ONLY: NO `side-band-64k` (the
/// simplest correct subset, RESEARCH §3 — report-status then arrives on the main
/// band) and NO force capability — so a force update is not expressible by any
/// code path.
const RECEIVE_PACK_CAPS: &str = "report-status agent=caprun";

/// Fail-closed refname check for a command line: non-empty, no force `+`, no
/// space / NUL / LF (which would break the pkt-line framing).
fn validate_refname(refname: &str) -> Result<()> {
    if refname.is_empty() {
        bail!("git.push: empty refname");
    }
    if refname.starts_with('+') {
        bail!("git.push: refname must not carry a force '+'");
    }
    if refname.bytes().any(|b| b == b' ' || b == 0 || b == b'\n') {
        bail!("git.push: refname contains an illegal byte (space/NUL/LF)");
    }
    Ok(())
}

/// Build the receive-pack command-list body (the pkt-line command line + a
/// terminating flush) from a caller-supplied `{old_oid, new_oid, refname}`.
///
/// STRUCTURAL DENIAL (DESIGN §1.3, RESEARCH §5 layer 2): refuses — for ANY input
/// — to construct a line whose `new_oid` is the zero-oid (a DELETE). This is
/// unreachable even via a human confirm. A CREATE is DISTINGUISHED and ALLOWED
/// (WG-6): `old_oid` MAY be the zero-oid (ref not advertised) as long as
/// `new_oid` is non-zero — the refusal keys on `new_oid == zero-oid` ONLY.
///
/// `old_oid` is a CALLER-supplied parameter sourced from the frozen info/refs
/// advertisement (Plan 44-03), NEVER read from the untrusted local repo
/// (WG-6/T-44-07). The capability set (`RECEIVE_PACK_CAPS`) carries no force
/// capability, so a force update is not expressible.
pub(crate) fn build_command_list(old_oid: &str, new_oid: &str, refname: &str) -> Result<Vec<u8>> {
    // Layer 2 structural denial: a delete is a command whose new-oid is zero.
    if is_zero_oid(new_oid) {
        bail!("git.push: refusing to build a deletion command (new-oid is the zero-oid)");
    }
    validate_oid(new_oid)?;
    // old_oid may legitimately be the zero-oid (a create) — validate_oid accepts it.
    validate_oid(old_oid)?;
    validate_refname(refname)?;

    let payload = format!("{old_oid} {new_oid} {refname}\0{RECEIVE_PACK_CAPS}");
    let mut out = pkt_line(payload.as_bytes());
    out.extend_from_slice(flush_pkt());
    Ok(out)
}

// ---- confined pack generation (Pattern B, RESEARCH §2 step 2/4, A14) ----

/// The git config/hook neutralization triple for a confined `git` child — the
/// SAME env-only neutralization `git.commit` uses (`git_commit.rs`): strips the
/// system + global config so no ambient alias/hook fires, and silences any
/// terminal credential prompt. NON-SECRET constants (the child stays
/// `env_clear()`ed otherwise, inheriting no `CAPRUN_GIT_PUSH_TOKEN`).
const GIT_NEUTRALIZE_ENV: [(&str, &str); 3] = [
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_TERMINAL_PROMPT", "0"),
];

/// Build the `git pack-objects --revs --stdout` stdin rev-list (RESEARCH §2 step
/// 4). For an UPDATE the range is `<new>\n^<old>\n` (pack only the objects the
/// remote lacks); for a CREATE (`old_oid` is the zero-oid — the ref is not
/// advertised, WG-6) the `^<old>` exclusion line is OMITTED so the full history
/// reachable from `<new>` is packed. Pure, host-portable.
fn build_pack_revlist(old_oid: &str, new_oid: &str) -> String {
    if is_zero_oid(old_oid) {
        format!("{new_oid}\n")
    } else {
        format!("{new_oid}\n^{old_oid}\n")
    }
}

/// Generate the binary packfile for `{old_oid}..{new_oid}` as a Pattern-B confined
/// child (RESEARCH §2 step 4, A14). Runs
/// `git -c core.hooksPath=/dev/null pack-objects --revs --stdout --thin
/// --delta-base-offset` under the SAME confinement as `git.commit`:
///   * net-denied (the unchanged `exec_child_filter` — a pack-gen child NEVER
///     opens `AF_INET`/`AF_INET6`; the pin is application-layer, the seccomp
///     net-deny is the §1.8 fail-closed backstop, T-44-09),
///   * Landlocked to the workspace root (reads `.git`),
///   * git-config-neutralized (`-c core.hooksPath=/dev/null` on the argv +
///     `GIT_CONFIG_NOSYSTEM=1`/`GIT_CONFIG_GLOBAL=/dev/null` in the env) so the
///     untrusted workspace `.git/config` RCE surface stays contained (T-44-11),
///   * `env_clear()`ed (inherits no `CAPRUN_GIT_PUSH_TOKEN` / broker secret).
/// The stdin is the tiny text rev-list; stdout is the RAW binary pack captured
/// intact via the WG-2 `run_launcher_capture_bytes` (never lossy). A non-zero
/// `pack-objects` exit is a fail-closed `Err` (exit-code gated, A14).
#[cfg(target_os = "linux")]
async fn generate_pack(
    workspace_root: &WorkspaceRoot,
    old_oid: &str,
    new_oid: &str,
) -> Result<Vec<u8>> {
    let revlist = build_pack_revlist(old_oid, new_oid);
    let argv: Vec<String> = vec![
        "-c".into(),
        "core.hooksPath=/dev/null".into(),
        "pack-objects".into(),
        "--revs".into(),
        "--stdout".into(),
        "--thin".into(),
        "--delta-base-offset".into(),
    ];
    let args_json = serde_json::to_string(&argv)
        .map_err(|e| anyhow!("git.push: failed to serialize pack-objects argv: {e}"))?;
    let cwd = workspace_root.root_path().to_string_lossy().into_owned();
    let launcher_path = resolve_launcher_path()?;

    let (exit_status, pack, stderr) = run_launcher_capture_bytes(
        &launcher_path,
        "git",
        &args_json,
        Some(cwd.as_str()),
        workspace_root,
        &GIT_NEUTRALIZE_ENV,
        Some(revlist.as_bytes()),
    )
    .await?;

    if !exit_status.success() {
        // Fail-closed: a non-zero pack-objects exit means NO valid pack was
        // produced. stderr is captured separately (never merged into the pack)
        // and folded into the error for diagnosis — the caller scrubs any log.
        bail!(
            "git.push: git pack-objects exited non-zero ({exit_status}); no pack generated: {}",
            String::from_utf8_lossy(&stderr)
        );
    }
    if pack.is_empty() {
        bail!("git.push: git pack-objects produced an empty pack (fail-closed)");
    }
    Ok(pack)
}

/// macOS no-op stub — the real `git pack-objects` leg is Linux-only (CLAUDE.md);
/// exercised on the Linux gate / compose-verify.
#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
async fn generate_pack(
    _workspace_root: &WorkspaceRoot,
    _old_oid: &str,
    _new_oid: &str,
) -> Result<Vec<u8>> {
    bail!("git.push pack generation is Linux-only (macOS no-op stub); exercised on the Linux gate")
}

/// Resolve the local source ref to its commit oid via a confined
/// `git rev-parse --verify <ref>^{{commit}}` child (RESEARCH §2 step 2). TEXT
/// output, so the existing String `run_launcher` fits verbatim; same config
/// neutralization + workspace cwd as `generate_pack`. Returns the validated oid.
/// A non-zero exit (unknown ref) is a fail-closed `Err`. The resolved oid is the
/// anti-TOCTOU comparand: the driver refuses if it != the human-confirmed frozen
/// oid (WG-7, DESIGN §1.6).
#[cfg(target_os = "linux")]
async fn resolve_new_oid(workspace_root: &WorkspaceRoot, src_ref: &str) -> Result<String> {
    let argv: Vec<String> = vec![
        "-c".into(),
        "core.hooksPath=/dev/null".into(),
        "rev-parse".into(),
        "--verify".into(),
        format!("{src_ref}^{{commit}}"),
    ];
    let args_json = serde_json::to_string(&argv)
        .map_err(|e| anyhow!("git.push: failed to serialize rev-parse argv: {e}"))?;
    let cwd = workspace_root.root_path().to_string_lossy().into_owned();
    let launcher_path = resolve_launcher_path()?;

    let (exit_status, output) = run_launcher(
        &launcher_path,
        "git",
        &args_json,
        Some(cwd.as_str()),
        workspace_root,
        &argv,
        &GIT_NEUTRALIZE_ENV,
    )
    .await?;

    if !exit_status.success() {
        bail!("git.push: git rev-parse could not resolve {src_ref:?} (fail-closed): {output}");
    }
    let oid = output.trim().to_string();
    validate_oid(&oid)?;
    Ok(oid)
}

/// macOS no-op stub — the real `git rev-parse` leg is Linux-only (CLAUDE.md).
#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
async fn resolve_new_oid(_workspace_root: &WorkspaceRoot, _src_ref: &str) -> Result<String> {
    bail!("git.push rev-parse is Linux-only (macOS no-op stub); exercised on the Linux gate")
}

/// Freeze the human-confirmed new-oid for a `git.push` refspec (WG-7, DESIGN
/// §1.6) — the SINGLE freeze entry point consumed by BOTH the server.rs
/// clean-Allowed confirm-gate arm AND the tainted-remote/refspec I2-Block insert
/// (Plan 44-04 Task 2), so neither can insert a pending git.push without a
/// frozen oid. Splits the refspec into `(<src>, <dst>)` and resolves the LOCAL
/// `<src>` ref to its commit oid via the confined `resolve_new_oid` rev-parse
/// child. The force/deletion refusal (`validate_git_refspec`) runs later in the
/// precheck/transfer path; this only needs the `<src>` name to resolve. Linux
/// confined child; the macOS host stubs `resolve_new_oid` (bails) — exercised on
/// the Linux gate. Appends no event, mints nothing.
pub async fn freeze_new_oid(workspace_root: &WorkspaceRoot, refspec: &str) -> Result<String> {
    let (src_ref, _dst_ref) = split_refspec(refspec);
    resolve_new_oid(workspace_root, src_ref).await
}

// ---- broker-env credential custody (RESEARCH A9, DESIGN §1.4) ----

/// The broker-local env var carrying the OPTIONAL push credential (DESIGN §1.4).
/// Read ONLY here, ONLY from the broker's own process env — NEVER a plan arg /
/// `ValueNode` / audit literal / `PendingConfirmation` / child env / planner.
const GIT_PUSH_TOKEN_ENV: &str = "CAPRUN_GIT_PUSH_TOKEN";

/// Serializes any test that mutates the process-global `CAPRUN_GIT_PUSH_TOKEN`
/// env var (mirrors `http_write::HTTP_WRITE_ENV_LOCK` / `github_pr::GITHUB_ENV_LOCK`).
#[cfg(test)]
pub(crate) static GIT_PUSH_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Read the OPTIONAL push credential from the broker's LOCAL process env ONLY
/// (mirror `http_write::write_bearer`, RESEARCH A9). Returns `None` when unset —
/// a push may legitimately need no credential, so this does NOT fail closed.
/// NEVER read from a `ValueNode`, a plan-node arg, the audit DB, or
/// `PendingConfirmation`; NEVER handed to the confined pack-gen child (which is
/// `env_clear()`ed); set ONLY on the receive-pack POST and NEVER followed across
/// a redirect (the driver refuses a 3xx).
fn git_push_token() -> Option<String> {
    std::env::var(GIT_PUSH_TOKEN_ENV).ok()
}

// ---- distinct git.push host allowlist (WG-9, DESIGN §1.5) ----

/// The DISTINCT `git.push` receive-pack host allowlist (WG-9) — SEPARATE from
/// both the GET `HOST_ALLOWLIST` and the `WRITE_HOST_ALLOWLIST` in
/// `http_request.rs`: a GET-readable or POST-writable host is NOT implicitly
/// push-target-able. Like those it is a broker-local trusted-config SECURITY
/// PROPERTY (an operator-surfaced deployment constant), never runtime-configurable
/// from a plan node / `ValueNode` / audit DB.
///
/// It ships EMPTY (fail-closed): the release build can push to NOTHING until an
/// operator surfaces a receive-pack target here — the maximally fail-closed
/// default. Under the NON-DEFAULT `mock-egress-ca` feature the Phase-46 mock
/// receive-pack host is ADDITIONALLY admitted so the composed live-proof can push
/// over the local TLS mock; that host is ABSENT from every production/default
/// build (see the `not(mock-egress-ca)` invariant test below).
const GIT_PUSH_HOST_ALLOWLIST: &[&str] = &[];

/// NON-DEFAULT `mock-egress-ca` feature ONLY: the single extra test host the
/// Phase-46 composed live-proof pushes to over the local TLS mock (the SAME host
/// `http_request.rs`'s `MOCK_EGRESS_HOST` names). Compiled OUT of — and thus
/// absent from — every production/default build.
#[cfg(feature = "mock-egress-ca")]
const MOCK_GIT_PUSH_HOST: &str = "github-mock.caprun.test";

/// True iff `host` is on the DISTINCT `GIT_PUSH_HOST_ALLOWLIST` (case-insensitive).
/// A non-push-allowlisted host is rejected by the transfer driver BEFORE any DNS
/// resolve (fail-closed, WG-9). Pure, host-portable. Mirrors
/// `http_request::is_write_host_allowlisted` but over the SEPARATE push list.
/// The default/release push allowlist is empty ONLY; under the NON-DEFAULT
/// `mock-egress-ca` feature the SINGLE `MOCK_GIT_PUSH_HOST` is additionally
/// accepted (the only push-egress relaxation the feature makes).
// Consumed by the Linux transfer driver + the host-portable cred tests; on a
// non-test macOS build it is unreferenced (the driver's gate call sits in the
// host-portable wrapper, but no non-test caller exists until Plan 44-04 wires it).
fn is_git_push_host_allowlisted(host: &str) -> bool {
    if GIT_PUSH_HOST_ALLOWLIST
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(host))
    {
        return true;
    }
    #[cfg(feature = "mock-egress-ca")]
    if MOCK_GIT_PUSH_HOST.eq_ignore_ascii_case(host) {
        return true;
    }
    false
}

// ---- credential/URL log scrub (DESIGN §1.4 MINOR-4, Phase-43 NIT-1) ----

/// Redact any `<scheme>://<userinfo>@…` substring in `msg` — the generic
/// credential-in-URL leak vector (a `https://x-access-token:TOKEN@host/…` string
/// in a transport error). Replaces the `userinfo@` run (between `://` and the
/// next `@` that precedes the path/query/end) with `[redacted-userinfo]@`. Pure,
/// host-portable.
fn strip_userinfo_urls(msg: &str) -> String {
    let mut out = String::with_capacity(msg.len());
    let bytes = msg.as_bytes();
    let mut i = 0usize;
    while i < msg.len() {
        if msg[i..].starts_with("://") {
            out.push_str("://");
            i += 3;
            // Scan the authority up to the first path/query/fragment/space
            // delimiter; if it contains an `@`, the part before the LAST `@` is
            // userinfo — redact it.
            let start = i;
            let mut j = i;
            while j < msg.len() {
                let c = bytes[j];
                if c == b'/' || c == b'?' || c == b'#' || c == b' ' || c == b'"' {
                    break;
                }
                j += 1;
            }
            let authority = &msg[start..j];
            if let Some(at) = authority.rfind('@') {
                out.push_str("[redacted-userinfo]@");
                out.push_str(&authority[at + 1..]);
            } else {
                out.push_str(authority);
            }
            i = j;
        } else {
            let ch = msg[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Scrub every credential/remote-URL leak vector from a status/error string
/// BEFORE it reaches the `eprintln!` logger (DESIGN §1.4 MINOR-4 — the broker-log
/// leak vector; folds Phase-43 NIT-1). Strips: the exact push token, the exact
/// remote URL substring, and any generic `<scheme>://<userinfo>@` credential-in-URL
/// material. Pure, host-portable.
fn scrub_secrets(msg: &str, token: Option<&str>, remote: &str) -> String {
    let mut out = strip_userinfo_urls(msg);
    if let Some(t) = token {
        if !t.is_empty() {
            out = out.replace(t, "[redacted-credential]");
        }
    }
    if !remote.is_empty() {
        out = out.replace(remote, "[redacted-remote]");
    }
    out
}

// ---- opaque two-phase audit (RESEARCH A9, DESIGN §1.4, P33/P34) ----

/// Fold a git.push transfer outcome into the OPAQUE two-phase audit (clone of
/// `http_write::append_write_outcome`, RESEARCH A9). On `Ok(())`: append
/// `git_push_succeeded`. On `Err`: route the SCRUBBED error to `eprintln!` (the
/// ONLY place it may appear — never the token/remote-URL/refspec/pack, MINOR-4),
/// append an OPAQUE `git_push_failed` event FIRST (terminal EVENT before any
/// terminal disposition — P33/P34), then propagate a non-swallowed `Err`. NO
/// retry. Both events carry NO remote URL, token, refspec literal, or pack bytes
/// in the hashed payload — only `effect_id` (in the `actor`) + a static
/// `event_type` marker (T-44-10). This module mints NOTHING (Gate 3 byte-identical).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn append_push_outcome(
    conn: &Connection,
    key: &[u8],
    session_id: Uuid,
    effect_id: Uuid,
    parent_id: Uuid,
    parent_hash: &str,
    push_result: Result<()>,
    token: Option<&str>,
    remote: &str,
) -> Result<(Uuid, String)> {
    match push_result {
        Ok(()) => {
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:git.push:{effect_id}"),
                "git_push_succeeded".into(),
                Utc::now(),
                vec![],
            );
            let hash = append_event(conn, key, &event, Some(parent_hash))
                .context("append git_push_succeeded")?;
            Ok((event.id, hash))
        }
        Err(e) => {
            // The raw error may embed the remote URL and/or a credential-in-URL
            // (reqwest errors carry the URL); SCRUB before the logger sees it.
            let scrubbed = scrub_secrets(&format!("{e:#}"), token, remote);
            eprintln!("[brokerd] git.push failed (effect_id={effect_id}): {scrubbed}");
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:git.push:{effect_id}"),
                "git_push_failed".into(),
                Utc::now(),
                vec![],
            );
            append_event(conn, key, &event, Some(parent_hash))
                .context("append git_push_failed")?;
            // Propagate a NON-swallowed, already-scrubbed error (no token/URL).
            Err(anyhow!("git.push transfer failed (effect_id={effect_id}): {scrubbed}"))
        }
    }
}

// ---- the frozen-IP two-request transfer driver (DESIGN §1.5/§1.6, WG-7) ----

/// Look up a required named literal from a frozen `ResolvedArg` snapshot,
/// fail-closed if missing OR empty (mirror `http_write::required_literal`).
fn required_literal<'a>(resolved_args: &'a [ResolvedArg], name: &str) -> Result<&'a str> {
    let literal = resolved_args
        .iter()
        .find(|a| a.name == name)
        .map(|a| a.literal.as_str())
        .ok_or_else(|| anyhow!("git.push: missing required `{name}` arg (fail-closed)"))?;
    if literal.trim().is_empty() {
        bail!("git.push: required `{name}` arg is empty (fail-closed)");
    }
    Ok(literal)
}

/// The frozen, validated PRE-PUSH inputs for a `git.push` confirm-release
/// dispatch (mirrors `http_write::PreparedWrite` / `github_pr::PreparedPr`).
/// Owned so a caller can validate independently of dispatch.
pub(crate) struct PreparedGitPush {
    /// The push remote URL literal (full https/allowlist/SSRF-pin vetting also
    /// re-runs in the transfer driver; here it is checked constructible +
    /// allowlisted).
    remote: String,
    /// The already-validated push refspec (force/deletion refused).
    refspec: String,
    /// The human-confirmed frozen new-oid (WG-7 anti-TOCTOU comparand).
    frozen_new_oid: String,
}

/// The fallible, SOCKET-FREE PRE-PUSH preparation shared by the Plan 44-04
/// Step-4.8d confirm-release precheck AND (through the SAME validators) the
/// transfer driver's own fail-closed gates (`run_git_push`): the `remote`/
/// `refspec` args present + non-empty, the `remote` URL constructible via
/// `http_request::validate_url` (https-only / no userinfo / vetted port), the
/// host on the DISTINCT `GIT_PUSH_HOST_ALLOWLIST` (WG-9), the refspec through
/// the force/deletion `validate_git_refspec` value-gate, and a non-empty,
/// well-formed `frozen_new_oid` (WG-7). Opens NO socket, resolves NO DNS,
/// appends NO event — pure/read-only, so the precheck (fail-closed-RECOVERABLE)
/// and the dispatch validate IDENTICALLY and cannot drift (the P33/P34
/// audit-gap discipline; mirror `http_write::prepare_http_write`).
pub(crate) fn prepare_git_push(
    resolved_args: &[ResolvedArg],
    frozen_new_oid: &str,
) -> Result<PreparedGitPush> {
    let remote = required_literal(resolved_args, "remote")?;
    let refspec = required_literal(resolved_args, "refspec")?;

    // The SAME url + DISTINCT-allowlist + refspec gates the transfer driver's
    // `run_git_push` applies — precheck and dispatch cannot drift.
    let host = http_request::validate_url(remote)?;
    if !is_git_push_host_allowlisted(&host) {
        bail!("git.push: host {host:?} is not on the git.push host allowlist (fail-closed)");
    }
    validate_git_refspec(refspec)?;

    // The human-confirmed frozen oid must be present + shape-valid (WG-7): an
    // empty or malformed oid can never match a live rev-parse, so it is
    // fail-closed-refused here BEFORE the one-shot confirmation is burned.
    if frozen_new_oid.trim().is_empty() {
        bail!("git.push: frozen_new_oid is empty (fail-closed — no payload to freeze)");
    }
    validate_oid(frozen_new_oid)?;

    Ok(PreparedGitPush {
        remote: remote.to_string(),
        refspec: refspec.to_string(),
        frozen_new_oid: frozen_new_oid.to_string(),
    })
}

/// Split a refspec into `(<src>, <dst>)` — the local ref whose new-oid is
/// resolved, and the remote refname the command-list updates + the old-oid is
/// looked up for. A bare `<ref>` (no `:`) uses the same name for both. Pure,
/// host-portable; the force/deletion refusal already ran in `validate_git_refspec`.
fn split_refspec(refspec: &str) -> (&str, &str) {
    match refspec.split_once(':') {
        Some((src, dst)) => (src, dst),
        None => (refspec, refspec),
    }
}

/// The WG-7 anti-TOCTOU equality gate (DESIGN §1.6): the human confirmed a
/// SPECIFIC new-oid; only that oid may be packed/pushed. A live rev-parse that
/// differs (a worker advanced the ref between Block and confirm) is a fail-closed
/// refusal. Pure, host-portable. Called AFTER the advertisement GET + rev-parse
/// but BEFORE the command-list / pack / POST — so a mismatch attempts NO push.
fn assert_frozen_oid(live_oid: &str, frozen_new_oid: &str) -> Result<()> {
    if live_oid != frozen_new_oid {
        bail!(
            "git.push: live new-oid does not match the human-confirmed frozen oid \
             (anti-TOCTOU refusal, WG-7/DESIGN §1.6)"
        );
    }
    Ok(())
}

/// The confirm-release-only `git.push` transfer driver (RESEARCH A9, DESIGN
/// §1.5/§1.6/§1.7). The SINGLE transfer entry point — there is NO auto-dispatch
/// Allowed variant because git.push is ALWAYS confirm-gated (DESIGN §1.6/§1.7);
/// it is called only from the confirm-release Step-7 dispatch (Plan 44-04).
///
/// Mirrors `http_write::invoke_http_write_from_resolved`'s conn/audit/parent-chain
/// shape (conn pre-locked; `append_push_outcome` folds EVERY failure into a
/// terminal `git_push_failed` FIRST, then propagates a non-swallowed Err —
/// P33/P34). It threads the human-confirmed `frozen_new_oid` (from the pending
/// confirmation, Plan 44-04) into the WG-7 equality gate. Mints NOTHING.
///
/// # Arguments
/// * `conn` — pre-locked broker audit connection (confirm-release is single-shot).
/// * `frozen_new_oid` — the human-confirmed new-oid (anti-TOCTOU comparand, WG-7).
/// * `workspace_root` — the workspace the confined pack-gen / rev-parse children
///   read `.git` from.
///
/// # Returns `(git_push_succeeded event_id, hash)` on a clean report-status.
#[allow(clippy::too_many_arguments)]
pub async fn invoke_git_push_from_resolved(
    conn: &Connection,
    key: &[u8],
    session_id: Uuid,
    effect_id: Uuid,
    resolved_args: &[ResolvedArg],
    workspace_root: &WorkspaceRoot,
    frozen_new_oid: &str,
    parent_id: Uuid,
    parent_hash: &str,
) -> Result<(Uuid, String)> {
    // Freeze the credential + remote up front for the scrub (even a pre-transfer
    // failure logs through the scrubbed audit fold).
    let token = git_push_token();
    let remote = required_literal(resolved_args, "remote").ok().map(str::to_string);
    let refspec = required_literal(resolved_args, "refspec").ok().map(str::to_string);
    let remote_for_scrub = remote.clone().unwrap_or_default();

    // Run the transfer; EVERY fallible leg (missing arg, url, allowlist, resolve,
    // GET, oid-mismatch, refspec, command-list, pack, POST, report-status) folds
    // into the SAME Result → a terminal `git_push_failed` FIRST (never a dangling
    // success), then a scrubbed non-swallowed Err (P33/P34).
    let push_result = run_git_push(
        remote.as_deref(),
        refspec.as_deref(),
        workspace_root,
        frozen_new_oid,
        token.as_deref(),
    )
    .await;

    append_push_outcome(
        conn,
        key,
        session_id,
        effect_id,
        parent_id,
        parent_hash,
        push_result,
        token.as_deref(),
        &remote_for_scrub,
    )
}

/// The transfer body: host-portable fail-closed gates (Err BEFORE any resolve),
/// then the Linux-gated frozen-IP network + confined-child legs. Kept separate
/// from the audit fold so a pre-resolve gate failure and a transport failure share
/// ONE `Result` path.
async fn run_git_push(
    remote: Option<&str>,
    refspec: Option<&str>,
    workspace_root: &WorkspaceRoot,
    frozen_new_oid: &str,
    token: Option<&str>,
) -> Result<()> {
    let remote = remote.ok_or_else(|| anyhow!("git.push: missing `remote` arg (fail-closed)"))?;
    let refspec =
        refspec.ok_or_else(|| anyhow!("git.push: missing `refspec` arg (fail-closed)"))?;

    // Fail-closed gates BEFORE any DNS resolve or socket (DESIGN §1.5, WG-9):
    //   validate_url (userinfo/non-https/port/IP-encoding) → the DISTINCT git.push
    //   host allowlist → the refspec force/deletion value-gate.
    let host = http_request::validate_url(remote)?;
    if !is_git_push_host_allowlisted(&host) {
        bail!("git.push: host {host:?} is not on the git.push host allowlist (fail-closed)");
    }
    validate_git_refspec(refspec)?;

    run_git_push_network(remote, refspec, &host, workspace_root, frozen_new_oid, token).await
}

/// Linux: the frozen-IP two-request exchange + confined-child pack legs (DESIGN
/// §1.5/§1.6). Resolves ONE SSRF-vetted `SocketAddr` (`resolve_and_vet`, Plan
/// 44-02), builds ONE redirect-none `build_pinned_client`, and issues BOTH the
/// info/refs GET and the receive-pack POST through it — `invoke_pinned_post`
/// (which re-resolves) is NEVER used (RESEARCH A7, DESIGN §1.5). Threads the
/// WG-7 frozen-oid refusal. Credential is set ONLY on the POST, never followed
/// across a redirect (redirect-none refuses a 3xx as a non-success status).
#[cfg(target_os = "linux")]
async fn run_git_push_network(
    remote: &str,
    refspec: &str,
    host: &str,
    workspace_root: &WorkspaceRoot,
    frozen_new_oid: &str,
    token: Option<&str>,
) -> Result<()> {
    // ONE frozen IP; ONE redirect-none client both requests ride (no re-resolve).
    let pinned = resolve_and_vet(host).await?;
    let client = build_pinned_client(host, pinned)?;

    let (src_ref, dst_ref) = split_refspec(refspec);
    let base = remote.trim_end_matches('/');

    // 1. info/refs GET (git-receive-pack advertisement) through the frozen client.
    let info_url = format!("{base}/info/refs?service=git-receive-pack");
    let adv_bytes = get_capped(&client, &info_url).await?;
    let adv = parse_advertisement(&adv_bytes)?;
    // Advertised old-oid for the target remote ref, or the zero-oid CREATE (WG-6:
    // the ref is not advertised). The refusal keys on new-oid==zero only, so a
    // create's zero old-oid is allowed by `build_command_list`.
    let old_oid = adv
        .old_oid_for(dst_ref)
        .map(str::to_string)
        .unwrap_or_else(|| ZERO_OID_SHA1.to_string());

    // 2. resolve the live local new-oid + the WG-7 anti-TOCTOU equality gate.
    //    A mismatch refuses HERE — before any command-list / pack / POST.
    let live_oid = resolve_new_oid(workspace_root, src_ref).await?;
    assert_frozen_oid(&live_oid, frozen_new_oid)?;

    // 3. structural-denial command-list (force/delete unreachable) + 4. pack from
    //    the confirmed {advertised old-oid, frozen new-oid} range.
    let command_list = build_command_list(&old_oid, frozen_new_oid, dst_ref)?;
    let pack = generate_pack(workspace_root, &old_oid, frozen_new_oid).await?;

    // 5. receive-pack POST: body = command-list ++ PACK, credential ONLY here.
    let mut body = command_list;
    body.extend_from_slice(&pack);
    let post_url = format!("{base}/git-receive-pack");
    let report = post_receive_pack(&client, &post_url, body, token).await?;

    // 6. parse report-status fail-closed (any `ng`/non-clean `unpack` => Err).
    parse_report_status(&report)
}

/// macOS no-op stub — the real socket + `git` legs are Linux-only (CLAUDE.md);
/// exercised on the Linux gate / compose-verify (Plan 44-04/44-05).
#[cfg(not(target_os = "linux"))]
async fn run_git_push_network(
    _remote: &str,
    _refspec: &str,
    _host: &str,
    _workspace_root: &WorkspaceRoot,
    _frozen_new_oid: &str,
    _token: Option<&str>,
) -> Result<()> {
    bail!("git.push live transfer is Linux-only (macOS no-op stub); exercised on the Linux gate")
}

/// Linux: GET `url` through the frozen-IP client and return the byte-capped body.
/// A non-success status (incl. a redirect-none 3xx) is a fail-closed `Err`.
#[cfg(target_os = "linux")]
async fn get_capped(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let mut resp = client
        .get(url)
        .header("User-Agent", "caprun")
        .send()
        .await
        .map_err(|e| anyhow!("git.push: info/refs GET failed: {e}"))?;
    if !resp.status().is_success() {
        bail!(
            "git.push: info/refs GET returned non-success status {} (redirect refused)",
            resp.status().as_u16()
        );
    }
    read_body_capped(&mut resp).await
}

/// Linux: POST the receive-pack body (command-list ++ PACK) through the SAME
/// frozen-IP client. The credential (broker-env only) is set ONLY here via Basic
/// auth (`x-access-token:<token>` — the git-over-HTTPS token convention), never
/// followed across a redirect: a `git-receive-pack` 3xx surfaces as a non-success
/// status and is REFUSED (redirect-none).
#[cfg(target_os = "linux")]
async fn post_receive_pack(
    client: &reqwest::Client,
    url: &str,
    body: Vec<u8>,
    token: Option<&str>,
) -> Result<Vec<u8>> {
    let mut req = client
        .post(url)
        .header("Content-Type", "application/x-git-receive-pack-request")
        .header("Accept", "application/x-git-receive-pack-result")
        .header("User-Agent", "caprun")
        .body(body);
    if let Some(t) = token {
        req = req.basic_auth("x-access-token", Some(t));
    }
    let mut resp = req
        .send()
        .await
        .map_err(|e| anyhow!("git.push: receive-pack POST failed: {e}"))?;
    if !resp.status().is_success() {
        bail!(
            "git.push: receive-pack POST returned non-success status {} (redirect refused)",
            resp.status().as_u16()
        );
    }
    read_body_capped(&mut resp).await
}

/// Linux: stream a response body with the SAME fail-closed byte cap as the GET
/// egress (RESEARCH A6) — never `resp.text()` (unbounded buffer, and a pkt-line
/// body is not UTF-8).
#[cfg(target_os = "linux")]
async fn read_body_capped(resp: &mut reqwest::Response) -> Result<Vec<u8>> {
    let mut body: Vec<u8> = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| anyhow!("git.push: reading response body failed: {e}"))?
    {
        body.extend_from_slice(&chunk);
        check_body_cap(body.len(), MAX_RESPONSE_BODY_BYTES)?;
    }
    Ok(body)
}

#[cfg(test)]
mod pack {
    use super::*;

    // ---- build_pack_revlist (host-portable, RESEARCH §2 step 4) ----

    const OID_OLD: &str = "1111111111111111111111111111111111111111";
    const OID_NEW: &str = "2222222222222222222222222222222222222222";

    #[test]
    fn revlist_update_includes_negative_old_oid() {
        // An UPDATE packs `<new>` excluding `^<old>` (only the new objects).
        assert_eq!(build_pack_revlist(OID_OLD, OID_NEW), format!("{OID_NEW}\n^{OID_OLD}\n"));
    }

    #[test]
    fn revlist_create_omits_negative_old_oid() {
        // WG-6: a CREATE (old-oid == zero-oid) OMITS the `^<old>` exclusion — the
        // full history reachable from `<new>` is packed (SHA-1 and SHA-256 zero).
        assert_eq!(build_pack_revlist(ZERO_OID_SHA1, OID_NEW), format!("{OID_NEW}\n"));
        assert_eq!(build_pack_revlist(ZERO_OID_SHA256, OID_NEW), format!("{OID_NEW}\n"));
        // The zero-oid must NOT appear as a `^<old>` exclusion line.
        assert!(!build_pack_revlist(ZERO_OID_SHA1, OID_NEW).contains('^'));
    }

    // ---- generate_pack: exit-code gating (Linux-gated — real git child) ----
    //
    // Runs the real `git pack-objects` confined child; macOS stubs it, so this
    // shows 0 tests on the Mac host (cfg-linux-test-blindness) — expected.
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn generate_pack_nonzero_exit_is_err() {
        // A directory that is NOT a git repo makes `git pack-objects` exit
        // non-zero → fail-closed Err (never a bogus/empty "pack"). Exercises the
        // A14 exit-code gate through the WG-2 binary launcher.
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_genpack_norepo_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let result = generate_pack(
            &ws,
            "1111111111111111111111111111111111111111",
            "2222222222222222222222222222222222222222",
        )
        .await;

        assert!(result.is_err(), "pack-objects in a non-repo dir must fail closed");
        std::fs::remove_dir_all(&root).ok();
    }
}

#[cfg(test)]
mod cred {
    use super::*;

    // ---- git_push_token: OPTIONAL broker-env-only sourcing (DESIGN §1.4) ----

    #[test]
    fn git_push_token_is_none_when_unset() {
        let _guard = GIT_PUSH_ENV_LOCK.lock().unwrap();
        std::env::remove_var(GIT_PUSH_TOKEN_ENV);
        assert!(
            git_push_token().is_none(),
            "an absent push token is None (OPTIONAL), NOT a fail-closed Err"
        );
    }

    #[test]
    fn git_push_token_reads_broker_env_when_set() {
        let _guard = GIT_PUSH_ENV_LOCK.lock().unwrap();
        std::env::set_var(GIT_PUSH_TOKEN_ENV, "gp-tok-123");
        let got = git_push_token();
        std::env::remove_var(GIT_PUSH_TOKEN_ENV);
        assert_eq!(got.as_deref(), Some("gp-tok-123"));
    }

    // ---- GIT_PUSH_HOST_ALLOWLIST is DISTINCT from the GET/WRITE lists (WG-9) ----

    #[test]
    fn push_allowlist_is_distinct_from_read_and_write_hosts() {
        // A host that is GET-readable (`api.github.com`, the GET HOST_ALLOWLIST)
        // is NOT thereby push-target-able — the push list is separate and empty
        // in release. Proves a GET/POST-reachable host is not implicitly a push
        // target (T-43-05 / WG-9).
        assert!(
            !is_git_push_host_allowlisted("api.github.com"),
            "the GET-readable host must NOT be push-allowlisted (distinct lists)"
        );
        assert!(!is_git_push_host_allowlisted("evil.example.com"));
    }

    /// RELEASE / default build: the git.push host allowlist is EXACTLY empty — the
    /// mock receive-pack host is NOT push-target-able without the feature. Gated
    /// `not(mock-egress-ca)`: it asserts the feature-OFF invariant (a provable
    /// base set), which by definition does not hold once the feature is compiled in.
    #[cfg(not(feature = "mock-egress-ca"))]
    #[test]
    fn push_allowlist_default_build_is_empty_base_set() {
        assert!(
            GIT_PUSH_HOST_ALLOWLIST.is_empty(),
            "the release git.push allowlist must be the empty base set (fail-closed)"
        );
        assert!(
            !is_git_push_host_allowlisted("github-mock.caprun.test"),
            "the Phase-46 mock host is NOT push-allowlisted in a default build"
        );
    }

    /// FEATURE ON (`--features mock-egress-ca`): the mock receive-pack host is
    /// push-allowlisted; the release base set stays empty otherwise.
    #[cfg(feature = "mock-egress-ca")]
    #[test]
    fn push_allowlist_feature_admits_only_the_mock_host() {
        assert!(is_git_push_host_allowlisted("github-mock.caprun.test"));
        assert!(is_git_push_host_allowlisted("GITHUB-MOCK.CAPRUN.TEST")); // case-insensitive
        // The GET-readable host is still NOT push-allowlisted even with the feature.
        assert!(!is_git_push_host_allowlisted("api.github.com"));
    }
}

#[cfg(test)]
mod audit {
    use super::*;
    use crate::audit::{find_event_by_type, open_audit_db};

    const TEST_KEY: &[u8] = b"git-push-rs-unit-test-key-not-secret";

    fn seed_root() -> (rusqlite::Connection, Uuid, Uuid, String) {
        let conn = open_audit_db(":memory:").unwrap();
        let session_id = Uuid::new_v4();
        let root = Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "broker".into(),
            "confirm_granted".into(),
            Utc::now(),
            vec![],
        );
        let root_hash = append_event(&conn, TEST_KEY, &root, None).unwrap();
        (conn, session_id, root.id, root_hash)
    }

    // ---- scrub_secrets: the broker-log leak vector (DESIGN §1.4 MINOR-4) ----

    #[test]
    fn scrub_strips_token_remote_and_userinfo_url() {
        const TOKEN: &str = "gp_SUPERSECRETTOKEN1234567890";
        const REMOTE: &str = "https://github-mock.caprun.test/owner/repo.git";
        // A realistic transport error embedding the remote, a credential-in-URL,
        // and the bare token.
        let raw = format!(
            "POST failed for {REMOTE}: tried https://x-access-token:{TOKEN}@github-mock.caprun.test/owner/repo.git (auth {TOKEN})"
        );
        let scrubbed = scrub_secrets(&raw, Some(TOKEN), REMOTE);
        assert!(!scrubbed.contains(TOKEN), "the token must be scrubbed: {scrubbed}");
        assert!(!scrubbed.contains("gp_"), "no token prefix may survive: {scrubbed}");
        assert!(!scrubbed.contains(REMOTE), "the remote URL must be scrubbed: {scrubbed}");
        // The generic userinfo-in-URL run is redacted even independent of the
        // exact token/remote replacements.
        assert!(
            !scrubbed.contains("x-access-token:"),
            "the userinfo credential material must be redacted: {scrubbed}"
        );
    }

    #[test]
    fn scrub_userinfo_url_redacts_generic_credential_in_url() {
        // No exact token/remote handed in — the GENERIC scheme://userinfo@ scan
        // still redacts (defense-in-depth against an unexpected credential shape).
        let raw = "connect error to ftp://user:pass@internal.host/path then done";
        let scrubbed = scrub_secrets(raw, None, "");
        assert!(!scrubbed.contains("user:pass"), "userinfo must be redacted: {scrubbed}");
        assert!(scrubbed.contains("[redacted-userinfo]@internal.host/path"));
    }

    // ---- opaque two-phase audit: failed-event-FIRST, no sensitive payload ----

    #[test]
    fn append_push_outcome_failure_appends_opaque_git_push_failed_first() {
        const TOKEN: &str = "gp_PAYLOADSECRET_9x";
        const REMOTE: &str = "https://github-mock.caprun.test/secret-owner/secret-repo.git";
        const REFSPEC: &str = "refs/heads/secret-branch:refs/heads/secret-branch";
        let (conn, session_id, parent_id, parent_hash) = seed_root();
        let effect_id = Uuid::new_v4();

        // An error string carrying every sensitive substring — proves the audit
        // payload stays OPAQUE regardless of what the transfer error embedded.
        let err = anyhow!("push to {REMOTE} refspec {REFSPEC} with {TOKEN} rejected");
        let result = append_push_outcome(
            &conn,
            TEST_KEY,
            session_id,
            effect_id,
            parent_id,
            &parent_hash,
            Err(err),
            Some(TOKEN),
            REMOTE,
        );

        assert!(result.is_err(), "a transfer failure must propagate a non-swallowed Err");
        // The propagated error is itself scrubbed (no token/remote).
        let err_str = format!("{:#}", result.unwrap_err());
        assert!(!err_str.contains(TOKEN), "propagated Err must be scrubbed of the token");
        assert!(!err_str.contains(REMOTE), "propagated Err must be scrubbed of the remote");

        let failed = find_event_by_type(&conn, &session_id.to_string(), "git_push_failed")
            .unwrap()
            .expect("git_push_failed event must be durably appended FIRST");
        assert_eq!(failed.actor, format!("sink:git.push:{effect_id}"));
        assert_eq!(failed.parent_id, Some(parent_id));
        assert!(
            find_event_by_type(&conn, &session_id.to_string(), "git_push_succeeded")
                .unwrap()
                .is_none(),
            "no git_push_succeeded on the failure path"
        );
        assert!(failed.taint.is_empty(), "the opaque failure event carries empty taint");

        // Grep the RAW persisted payload for every sensitive literal — ALL absent.
        let payload: String = conn
            .query_row(
                "SELECT payload FROM events WHERE event_type = 'git_push_failed'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!payload.contains(TOKEN), "token must NEVER appear in the hashed payload");
        assert!(!payload.contains("gp_"), "no token prefix may leak into the payload");
        assert!(!payload.contains("secret-owner"), "the remote URL must NEVER be in the payload");
        assert!(!payload.contains(REFSPEC), "the refspec literal must NEVER be in the payload");
        assert!(!payload.contains("secret-branch"), "no refspec ref material in the payload");
    }

    #[test]
    fn append_push_outcome_success_appends_git_push_succeeded() {
        let (conn, session_id, parent_id, parent_hash) = seed_root();
        let effect_id = Uuid::new_v4();
        let (evt_id, hash) = append_push_outcome(
            &conn,
            TEST_KEY,
            session_id,
            effect_id,
            parent_id,
            &parent_hash,
            Ok(()),
            Some("gp_tok"),
            "https://github-mock.caprun.test/o/r.git",
        )
        .expect("a clean report-status must append git_push_succeeded");
        assert!(!hash.is_empty());

        let ok = find_event_by_type(&conn, &session_id.to_string(), "git_push_succeeded")
            .unwrap()
            .expect("git_push_succeeded event must exist");
        assert_eq!(ok.id, evt_id);
        assert_eq!(ok.actor, format!("sink:git.push:{effect_id}"));
        assert_eq!(ok.parent_id, Some(parent_id));
        assert!(ok.taint.is_empty(), "the opaque success event carries empty taint");
    }
}

#[cfg(test)]
mod transfer {
    use super::*;
    use crate::audit::{find_event_by_type, open_audit_db};
    use runtime_core::plan_node::{TaintLabel, ValueId};

    const TEST_KEY: &[u8] = b"git-push-transfer-rs-unit-test-key";
    const OID_FROZEN: &str = "2222222222222222222222222222222222222222";
    const OID_OTHER: &str = "3333333333333333333333333333333333333333";

    // ---- WG-7 anti-TOCTOU frozen-oid equality gate (host-portable) ----

    #[test]
    fn assert_frozen_oid_accepts_equal_refuses_differing() {
        assert!(assert_frozen_oid(OID_FROZEN, OID_FROZEN).is_ok());
        // A worker that advanced the ref between Block and confirm => refusal.
        assert!(assert_frozen_oid(OID_OTHER, OID_FROZEN).is_err());
    }

    // ---- refspec src/dst split (host-portable) ----

    #[test]
    fn split_refspec_src_dst_and_bare() {
        assert_eq!(
            split_refspec("refs/heads/main:refs/heads/prod"),
            ("refs/heads/main", "refs/heads/prod")
        );
        assert_eq!(split_refspec("refs/heads/main"), ("refs/heads/main", "refs/heads/main"));
    }

    // ---- audit-fold: a fail-closed gate before any resolve folds into a terminal
    // opaque git_push_failed FIRST, then a scrubbed Err (host-portable: the
    // git.push allowlist is empty in release, so ANY host is refused BEFORE the
    // Linux network legs — a POST is never attempted). ----

    fn resolved(remote: &str, refspec: &str) -> Vec<ResolvedArg> {
        let mk = |name: &str, literal: &str| ResolvedArg {
            name: name.to_string(),
            value_id: ValueId::new(),
            literal: literal.to_string(),
            taint: vec![TaintLabel::ExternalUntrusted],
            provenance_chain: vec![],
        };
        vec![mk("remote", remote), mk("refspec", refspec)]
    }

    #[tokio::test]
    async fn driver_non_allowlisted_host_folds_into_opaque_git_push_failed() {
        let conn = open_audit_db(":memory:").unwrap();
        let session_id = Uuid::new_v4();
        let root = Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "broker".into(),
            "confirm_granted".into(),
            Utc::now(),
            vec![],
        );
        let root_hash = append_event(&conn, TEST_KEY, &root, None).unwrap();
        let effect_id = Uuid::new_v4();

        // A well-formed https remote whose host is NOT on the (empty) git.push
        // allowlist — refused BEFORE any DNS resolve or socket.
        const REMOTE: &str = "https://not-on-allowlist.example.com/secret-owner/secret-repo.git";
        let mut root_dir = std::env::temp_dir();
        root_dir.push(format!("caprun_gp_driver_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root_dir).unwrap();
        let ws = WorkspaceRoot::open(&root_dir).unwrap();

        let result = invoke_git_push_from_resolved(
            &conn,
            TEST_KEY,
            session_id,
            effect_id,
            &resolved(REMOTE, "refs/heads/main:refs/heads/main"),
            &ws,
            OID_FROZEN,
            root.id,
            &root_hash,
        )
        .await;

        assert!(result.is_err(), "a non-allowlisted host must fail closed, never be swallowed");

        let failed = find_event_by_type(&conn, &session_id.to_string(), "git_push_failed")
            .unwrap()
            .expect("git_push_failed must be appended FIRST");
        assert_eq!(failed.actor, format!("sink:git.push:{effect_id}"));
        assert_eq!(failed.parent_id, Some(root.id));
        assert!(
            find_event_by_type(&conn, &session_id.to_string(), "git_push_succeeded")
                .unwrap()
                .is_none(),
            "no git_push_succeeded when the allowlist gate refuses"
        );

        // The persisted payload is opaque — no remote-URL material.
        let payload: String = conn
            .query_row(
                "SELECT payload FROM events WHERE event_type = 'git_push_failed'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!payload.contains("secret-owner"), "the remote URL must NEVER be in the payload");
        assert!(!payload.contains("not-on-allowlist"), "no remote host in the payload");

        std::fs::remove_dir_all(&root_dir).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- pkt-line encode/decode round-trip + malformed input ----

    #[test]
    fn pkt_line_encodes_4hex_length_prefix() {
        // "hi" (2 bytes) => length 2+4 = 6 => "0006hi".
        assert_eq!(pkt_line(b"hi"), b"0006hi");
        // empty payload => length 4 => "0004".
        assert_eq!(pkt_line(b""), b"0004");
    }

    #[test]
    fn flush_pkt_is_0000() {
        assert_eq!(flush_pkt(), b"0000");
    }

    #[test]
    fn pkt_read_decodes_data_then_flush_round_trip() {
        // Encode two data pkts + a flush, then decode them back in order.
        let mut stream = Vec::new();
        stream.extend_from_slice(&pkt_line(b"alpha"));
        stream.extend_from_slice(&pkt_line(b"beta"));
        stream.extend_from_slice(flush_pkt());

        let mut buf: &[u8] = &stream;
        assert_eq!(read_pkt(&mut buf).unwrap(), Some(Pkt::Data(b"alpha".to_vec())));
        assert_eq!(read_pkt(&mut buf).unwrap(), Some(Pkt::Data(b"beta".to_vec())));
        assert_eq!(read_pkt(&mut buf).unwrap(), Some(Pkt::Flush));
        // Buffer now exhausted → clean end of stream.
        assert_eq!(read_pkt(&mut buf).unwrap(), None);
    }

    #[test]
    fn pkt_read_flush_is_zero_length() {
        let mut buf: &[u8] = b"0000";
        assert_eq!(read_pkt(&mut buf).unwrap(), Some(Pkt::Flush));
        assert!(buf.is_empty());
    }

    #[test]
    fn pkt_read_rejects_non_hex_length() {
        let mut buf: &[u8] = b"zzzzpayload";
        assert!(read_pkt(&mut buf).is_err());
    }

    #[test]
    fn pkt_read_rejects_truncated_length_header() {
        let mut buf: &[u8] = b"00"; // only 2 of 4 header bytes
        assert!(read_pkt(&mut buf).is_err());
    }

    #[test]
    fn pkt_read_rejects_truncated_payload() {
        // Length says 0009 (5 payload bytes) but only 2 are present.
        let mut buf: &[u8] = b"0009hi";
        assert!(read_pkt(&mut buf).is_err());
    }

    #[test]
    fn pkt_read_rejects_invalid_short_nonflush_length() {
        // 0002 is a protocol-v2 special, not valid in this receive-pack subset.
        let mut buf: &[u8] = b"0002";
        assert!(read_pkt(&mut buf).is_err());
    }

    // ---- advertisement parse ----

    /// Build a realistic smart-HTTP receive-pack advertisement body.
    fn adv_body() -> Vec<u8> {
        let oid_main = "1111111111111111111111111111111111111111";
        let oid_dev = "2222222222222222222222222222222222222222";
        let mut body = Vec::new();
        body.extend_from_slice(&pkt_line(b"# service=git-receive-pack\n"));
        body.extend_from_slice(flush_pkt());
        // First ref line carries capabilities after a NUL.
        let first = format!(
            "{oid_main} refs/heads/main\0report-status delete-refs side-band-64k agent=git/2.40\n"
        );
        body.extend_from_slice(&pkt_line(first.as_bytes()));
        let second = format!("{oid_dev} refs/heads/dev\n");
        body.extend_from_slice(&pkt_line(second.as_bytes()));
        body.extend_from_slice(flush_pkt());
        body
    }

    #[test]
    fn parse_advertisement_reads_caps_and_refs() {
        let adv = parse_advertisement(&adv_body()).unwrap();
        assert!(adv.caps.contains(&"report-status".to_string()));
        assert!(adv.caps.contains(&"agent=git/2.40".to_string()));
        assert_eq!(
            adv.old_oid_for("refs/heads/main"),
            Some("1111111111111111111111111111111111111111")
        );
        assert_eq!(
            adv.old_oid_for("refs/heads/dev"),
            Some("2222222222222222222222222222222222222222")
        );
    }

    #[test]
    fn parse_advertisement_signals_create_for_unadvertised_ref() {
        // WG-6: a ref that is NOT advertised => None => the caller treats it as a
        // create (old-oid = zero-oid).
        let adv = parse_advertisement(&adv_body()).unwrap();
        assert_eq!(adv.old_oid_for("refs/heads/brand-new"), None);
    }

    #[test]
    fn parse_advertisement_handles_empty_repo_capabilities_line() {
        // An empty repo advertises a single "capabilities^{}" line with a zero
        // oid — no real ref, so any target ref is a create.
        let mut body = Vec::new();
        body.extend_from_slice(&pkt_line(b"# service=git-receive-pack\n"));
        body.extend_from_slice(flush_pkt());
        let line = format!("{ZERO_OID_SHA1} capabilities^{{}}\0report-status agent=git/2.40\n");
        body.extend_from_slice(&pkt_line(line.as_bytes()));
        body.extend_from_slice(flush_pkt());

        let adv = parse_advertisement(&body).unwrap();
        assert!(adv.caps.contains(&"report-status".to_string()));
        assert_eq!(adv.old_oid_for("refs/heads/main"), None); // create
    }

    #[test]
    fn parse_advertisement_rejects_unterminated_stream() {
        // Ref line with no terminating flush → fail closed.
        let mut body = Vec::new();
        body.extend_from_slice(&pkt_line(b"# service=git-receive-pack\n"));
        body.extend_from_slice(flush_pkt());
        let first = "1111111111111111111111111111111111111111 refs/heads/main\0report-status\n";
        body.extend_from_slice(&pkt_line(first.as_bytes()));
        // (no flush)
        assert!(parse_advertisement(&body).is_err());
    }

    #[test]
    fn parse_advertisement_rejects_malformed_oid() {
        let mut body = Vec::new();
        body.extend_from_slice(&pkt_line(b"# service=git-receive-pack\n"));
        body.extend_from_slice(flush_pkt());
        // Non-hex / wrong-length oid.
        body.extend_from_slice(&pkt_line(b"NOTANOID refs/heads/main\0report-status\n"));
        body.extend_from_slice(flush_pkt());
        assert!(parse_advertisement(&body).is_err());
    }

    // ---- report-status parse ----

    #[test]
    fn parse_report_status_accepts_clean_unpack_and_ref_ok() {
        let mut body = Vec::new();
        body.extend_from_slice(&pkt_line(b"unpack ok\n"));
        body.extend_from_slice(&pkt_line(b"ok refs/heads/main\n"));
        body.extend_from_slice(flush_pkt());
        assert!(parse_report_status(&body).is_ok());
    }

    #[test]
    fn parse_report_status_fails_on_unpack_error() {
        let mut body = Vec::new();
        body.extend_from_slice(&pkt_line(b"unpack index-pack failed\n"));
        body.extend_from_slice(flush_pkt());
        assert!(parse_report_status(&body).is_err());
    }

    #[test]
    fn parse_report_status_fails_on_ng_ref() {
        // T-44-08: a per-ref `ng` must fail closed, never be read as accepted.
        let mut body = Vec::new();
        body.extend_from_slice(&pkt_line(b"unpack ok\n"));
        body.extend_from_slice(&pkt_line(b"ng refs/heads/main non-fast-forward\n"));
        body.extend_from_slice(flush_pkt());
        assert!(parse_report_status(&body).is_err());
    }

    #[test]
    fn parse_report_status_fails_when_no_unpack_line() {
        // Missing `unpack ok` entirely → fail closed even with a ref ok present.
        let mut body = Vec::new();
        body.extend_from_slice(&pkt_line(b"ok refs/heads/main\n"));
        body.extend_from_slice(flush_pkt());
        assert!(parse_report_status(&body).is_err());
    }

    #[test]
    fn parse_report_status_fails_when_no_ref_status() {
        // `unpack ok` but zero per-ref status lines → fail closed.
        let mut body = Vec::new();
        body.extend_from_slice(&pkt_line(b"unpack ok\n"));
        body.extend_from_slice(flush_pkt());
        assert!(parse_report_status(&body).is_err());
    }

    #[test]
    fn parse_report_status_fails_on_unrecognized_line() {
        let mut body = Vec::new();
        body.extend_from_slice(&pkt_line(b"unpack ok\n"));
        body.extend_from_slice(&pkt_line(b"weird status line\n"));
        body.extend_from_slice(flush_pkt());
        assert!(parse_report_status(&body).is_err());
    }

    // ---- validate_git_refspec (structural denial layer 1) ----

    const OID_A: &str = "1111111111111111111111111111111111111111";
    const OID_B: &str = "2222222222222222222222222222222222222222";

    #[test]
    fn refspec_accepts_plain_src_dst_and_bare_ref() {
        assert!(validate_git_refspec("refs/heads/main:refs/heads/main").is_ok());
        assert!(validate_git_refspec("main:main").is_ok());
        assert!(validate_git_refspec("refs/heads/main").is_ok());
        assert!(validate_git_refspec("HEAD:refs/heads/main").is_ok());
    }

    #[test]
    fn refspec_rejects_leading_plus_force() {
        assert!(validate_git_refspec("+refs/heads/main:refs/heads/main").is_err());
        assert!(validate_git_refspec("+main").is_err());
    }

    #[test]
    fn refspec_rejects_force_flag_shaped_tokens() {
        assert!(validate_git_refspec("--force").is_err());
        assert!(validate_git_refspec("--force-with-lease").is_err());
        assert!(validate_git_refspec("--FORCE").is_err()); // case-insensitive
        assert!(validate_git_refspec("--delete").is_err()); // any --flag shape
    }

    #[test]
    fn refspec_rejects_deletion_empty_src() {
        // A bare ":dst" (empty <src>) deletes the remote ref — refused.
        assert!(validate_git_refspec(":refs/heads/main").is_err());
        assert!(validate_git_refspec(":main").is_err());
    }

    #[test]
    fn refspec_rejects_empty_or_malformed() {
        assert!(validate_git_refspec("").is_err());
        assert!(validate_git_refspec("main:").is_err()); // empty <dst>
        assert!(validate_git_refspec("a:b:c").is_err()); // multiple ':'
    }

    // ---- build_command_list (structural denial layer 2) ----

    #[test]
    fn command_list_builds_a_valid_update_line() {
        let body = build_command_list(OID_A, OID_B, "refs/heads/main").unwrap();
        // First pkt is the command line; caps ride after a NUL, terminated by flush.
        let mut buf: &[u8] = &body;
        let pkt = read_pkt(&mut buf).unwrap().unwrap();
        let line = match pkt {
            Pkt::Data(d) => d,
            Pkt::Flush => panic!("expected a data command line, got flush"),
        };
        let (refpart, caps) = split_ref_and_caps(&line);
        assert_eq!(
            std::str::from_utf8(refpart).unwrap(),
            format!("{OID_A} {OID_B} refs/heads/main")
        );
        // Fixed caps: report-status + agent, and crucially NO force/side-band cap.
        assert!(caps.contains(&"report-status".to_string()));
        assert!(caps.iter().all(|c| c != "side-band-64k"));
        assert!(caps.iter().all(|c| !c.contains("force")));
        // Terminated by a flush.
        assert_eq!(read_pkt(&mut buf).unwrap(), Some(Pkt::Flush));
    }

    #[test]
    fn command_list_refuses_zero_new_oid_delete() {
        // A zero new-oid is a delete — refused by construction (SHA-1 and SHA-256
        // widths), for any old-oid.
        assert!(build_command_list(OID_A, ZERO_OID_SHA1, "refs/heads/main").is_err());
        assert!(build_command_list(OID_A, ZERO_OID_SHA256, "refs/heads/main").is_err());
        assert!(build_command_list(ZERO_OID_SHA1, ZERO_OID_SHA1, "refs/heads/main").is_err());
    }

    #[test]
    fn command_list_allows_create_with_zero_old_oid() {
        // WG-6: a create is old-oid == zero-oid with a NON-zero new-oid — ALLOWED
        // (the refusal keys on new-oid only, distinguishing create from delete).
        let body = build_command_list(ZERO_OID_SHA1, OID_B, "refs/heads/brand-new").unwrap();
        let mut buf: &[u8] = &body;
        let pkt = read_pkt(&mut buf).unwrap().unwrap();
        let line = match pkt {
            Pkt::Data(d) => d,
            Pkt::Flush => panic!("expected a data command line"),
        };
        let (refpart, _caps) = split_ref_and_caps(&line);
        assert_eq!(
            std::str::from_utf8(refpart).unwrap(),
            format!("{ZERO_OID_SHA1} {OID_B} refs/heads/brand-new")
        );
    }

    #[test]
    fn command_list_rejects_malformed_oids_and_refnames() {
        assert!(build_command_list("NOTHEX", OID_B, "refs/heads/main").is_err());
        assert!(build_command_list(OID_A, "shortoid", "refs/heads/main").is_err());
        assert!(build_command_list(OID_A, OID_B, "").is_err());
        assert!(build_command_list(OID_A, OID_B, "+refs/heads/main").is_err());
        assert!(build_command_list(OID_A, OID_B, "refs/heads/ma in").is_err()); // space
    }
}

#[cfg(test)]
mod prepare {
    use super::*;
    use runtime_core::plan_node::{TaintLabel, ValueId};

    /// A valid 40-hex SHA-1 oid for the frozen-oid arg.
    const FROZEN_OID: &str = "abcdef0123456789abcdef0123456789abcdef01";

    fn arg(name: &str, literal: &str) -> ResolvedArg {
        ResolvedArg {
            name: name.to_string(),
            value_id: ValueId::new(),
            literal: literal.to_string(),
            taint: vec![TaintLabel::UserTrusted],
            provenance_chain: vec![],
        }
    }

    /// A well-formed `{remote, refspec}` set pointing at the mock push host —
    /// only allowlisted under the `mock-egress-ca` feature.
    fn mock_args() -> Vec<ResolvedArg> {
        vec![
            arg("remote", "https://github-mock.caprun.test/owner/repo.git"),
            arg("refspec", "refs/heads/main:refs/heads/main"),
        ]
    }

    // ── prepare_git_push negative gates (host-portable, default build) ──

    #[test]
    fn prepare_errs_on_missing_required_arg() {
        for missing in ["remote", "refspec"] {
            let mut args = mock_args();
            args.retain(|a| a.name != missing);
            assert!(
                prepare_git_push(&args, FROZEN_OID).is_err(),
                "a missing `{missing}` arg must fail closed"
            );
        }
    }

    #[test]
    fn prepare_errs_on_empty_frozen_new_oid() {
        assert!(
            prepare_git_push(&mock_args(), "   ").is_err(),
            "an empty frozen_new_oid must fail closed (no payload to freeze)"
        );
    }

    #[test]
    fn prepare_errs_on_malformed_frozen_new_oid() {
        assert!(
            prepare_git_push(&mock_args(), "not-a-valid-oid").is_err(),
            "a non-hex/short frozen_new_oid must fail closed (WG-7 shape gate)"
        );
    }

    #[test]
    fn prepare_errs_on_force_refspec() {
        // The SAME force/deletion value-gate the transfer driver applies — a
        // leading '+' (force) is refused in the precheck, no drift.
        let mut args = mock_args();
        for a in args.iter_mut() {
            if a.name == "refspec" {
                a.literal = "+refs/heads/main:refs/heads/main".to_string();
            }
        }
        assert!(prepare_git_push(&args, FROZEN_OID).is_err());
    }

    #[test]
    fn prepare_errs_on_deletion_refspec() {
        let mut args = mock_args();
        for a in args.iter_mut() {
            if a.name == "refspec" {
                a.literal = ":refs/heads/main".to_string();
            }
        }
        assert!(prepare_git_push(&args, FROZEN_OID).is_err());
    }

    #[test]
    fn prepare_errs_on_non_https_remote() {
        let mut args = mock_args();
        for a in args.iter_mut() {
            if a.name == "remote" {
                a.literal = "http://github-mock.caprun.test/owner/repo.git".to_string();
            }
        }
        assert!(prepare_git_push(&args, FROZEN_OID).is_err(), "a non-https remote must fail closed");
    }

    #[test]
    fn prepare_errs_on_non_allowlisted_host() {
        // A GET-readable host (`api.github.com`) is NOT push-allowlisted — the
        // DISTINCT git.push allowlist gate (WG-9) runs in the precheck too, so a
        // precheck cannot pass a host the transfer driver would reject.
        let mut args = mock_args();
        for a in args.iter_mut() {
            if a.name == "remote" {
                a.literal = "https://api.github.com/owner/repo.git".to_string();
            }
        }
        assert!(prepare_git_push(&args, FROZEN_OID).is_err());
    }

    // ── prepare_git_push positive (needs the mock host on the allowlist) ──

    /// FEATURE ON (`--features mock-egress-ca`): a well-formed `{remote,
    /// refspec}` at the allowlisted mock host with a valid frozen oid prepares
    /// OK. The default/release build has an EMPTY push allowlist (fail-closed),
    /// so this positive path is only reachable under the mock feature (mirrors
    /// the cred module's feature-gated positive test).
    #[cfg(feature = "mock-egress-ca")]
    #[test]
    fn prepare_ok_for_well_formed_mock_args() {
        assert!(
            prepare_git_push(&mock_args(), FROZEN_OID).is_ok(),
            "a well-formed allowlisted-host push must prepare OK under the mock feature"
        );
    }
}
