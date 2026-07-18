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
//! code). The confined `git pack-objects` PACK bytes + the two-request wire
//! driver (which CONSUMES these functions through the WG-1 frozen-IP client) are
//! Plan 44-03.
//!
//! Until Plan 44-03 wires the driver, these substrate functions are reachable
//! only from this module's own unit tests, so a non-test `cargo build` sees them
//! as unused — hence the module-scoped `allow(dead_code)`. It narrows to nothing
//! once 44-03's dispatch arm consumes them.
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

use anyhow::{anyhow, bail, Result};
use std::collections::BTreeMap;

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
