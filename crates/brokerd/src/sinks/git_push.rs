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
}
