//! Parent↔worker hold protocol (Phase 50 / CONFIRM-01 + CLI-02).
//!
//! # Product path (locked)
//!
//! Stay-connected parent-pipe only (DESIGN-multi-step-plan-stream.md §3
//! Option A). Reconnect-remint and dual-Session stitch are **rejected**
//! (§3.3). No broker `WaitForConfirm` IPC verb — main writes `PROCEED` /
//! `ABORT` on the worker's stdin after acting on the durable pending row.
//!
//! # Trust note for parents (Plan 02)
//!
//! A `BLOCKED` line is **not** success. The parent must only write
//! `PROCEED` after a human confirm release (`ConfirmOutcome::Released`),
//! never after merely parsing `BLOCKED`. `ABORT` on human deny / operator
//! abort. Forged protocol lines must never auto-authorize effects.
//!
//! `policy_deny` is carried as `DENIED code=policy_deny` on stdout; the
//! process exit integer shares **2** with other denied/aborted outcomes —
//! the `code=` field is the distinct machine label (CLI-02).
//!
//! # Purity
//!
//! This module is pure string transforms + exit mapping. Worker/main own
//! `println` / `read_line` / process exit.

use std::collections::HashMap;

/// Prefix on every worker→main machine-readable stream line.
pub const STREAM_PREFIX: &str = "caprun-stream:";

/// Machine-readable stream lines (worker → main on stdout).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamLine {
    /// Hold: durable pending exists; worker is waiting for PROCEED/ABORT.
    Blocked {
        effect_id: String,
        sink: String,
    },
    /// Abort remaining: deny / not-implemented / policy_deny.
    Denied {
        code: String,
        sink: String,
    },
    /// Optional progress after an Allowed decision.
    NodeAllowed {
        step: usize,
        sink: String,
    },
    /// Success terminal: at least one submit, stream exhausted.
    StreamDone {
        submitted: usize,
    },
}

/// Parent → worker hold resume tokens (exact trim match only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldResume {
    Proceed,
    Abort,
}

/// CLI-02 stream exit taxonomy (process exit integers).
///
/// | Code | Meaning |
/// |------|---------|
/// | 0 | Full success (incl. Block-released + remaining Allowed) |
/// | 2 | Denied / aborted (policy_deny, human ABORT, NotImplemented) |
/// | 3 | Blocked / hold incomplete (pending still open) |
/// | 1 | Usage / infra / empty stream / crash |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum StreamExitCode {
    Success = 0,
    Infra = 1,
    DeniedAborted = 2,
    BlockedIncomplete = 3,
}

impl StreamExitCode {
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

impl From<StreamExitCode> for i32 {
    fn from(c: StreamExitCode) -> i32 {
        c.as_i32()
    }
}

/// Terminal kind for pure exit mapping (no I/O, no process state).
///
/// Consumed by `map_stream_exit` (worker exit paths + main orchestration in
/// Plan 02). Public API of this module — not dead when only one bin imports
/// a subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // used by main (Plan 02) + unit tests; worker uses exit ints directly
pub enum StreamTerminalKind {
    /// Full Allowed stream (or Allowed after hold-release with remaining Allowed).
    Success,
    /// Denied / NotImplemented / human ABORT / policy_deny bucket.
    DeniedAborted,
    /// Block without release / hold incomplete.
    BlockedIncomplete,
    /// Empty stream, parse/spawn/crash, unknown hold token, infra.
    InfraOrEmpty,
}

/// Pure mapper: stream terminal → CLI-02 exit integer taxonomy.
#[allow(dead_code)] // used by main (Plan 02) + unit tests
pub fn map_stream_exit(terminal: StreamTerminalKind) -> StreamExitCode {
    match terminal {
        StreamTerminalKind::Success => StreamExitCode::Success,
        StreamTerminalKind::DeniedAborted => StreamExitCode::DeniedAborted,
        StreamTerminalKind::BlockedIncomplete => StreamExitCode::BlockedIncomplete,
        StreamTerminalKind::InfraOrEmpty => StreamExitCode::Infra,
    }
}

/// Format a machine-readable stream line (exact table in RESEARCH Pattern 2).
pub fn format_line(line: &StreamLine) -> String {
    match line {
        StreamLine::Blocked { effect_id, sink } => {
            format!("{STREAM_PREFIX} BLOCKED effect_id={effect_id} sink={sink}")
        }
        StreamLine::Denied { code, sink } => {
            format!("{STREAM_PREFIX} DENIED code={code} sink={sink}")
        }
        StreamLine::NodeAllowed { step, sink } => {
            format!("{STREAM_PREFIX} NODE_ALLOWED step={step} sink={sink}")
        }
        StreamLine::StreamDone { submitted } => {
            format!("{STREAM_PREFIX} STREAM_DONE submitted={submitted}")
        }
    }
}

/// Parse a machine-readable stream line. Whitespace-tolerant; fail-closed on garbage.
#[allow(dead_code)] // used by main orchestration (Plan 02) + unit tests
pub fn parse_line(raw: &str) -> Result<StreamLine, String> {
    let s = raw.trim();
    let rest = s
        .strip_prefix(STREAM_PREFIX)
        .ok_or_else(|| format!("missing `{STREAM_PREFIX}` prefix"))?
        .trim();

    let mut parts = rest.split_whitespace();
    let kind = parts
        .next()
        .ok_or_else(|| "empty stream line after prefix".to_string())?;

    let mut map: HashMap<&str, &str> = HashMap::new();
    for tok in parts {
        match tok.split_once('=') {
            Some((k, v)) if !k.is_empty() => {
                map.insert(k, v);
            }
            _ => return Err(format!("malformed token (expected key=value): {tok:?}")),
        }
    }

    match kind {
        "BLOCKED" => {
            let effect_id = require_key(&map, "effect_id")?.to_string();
            let sink = require_key(&map, "sink")?.to_string();
            Ok(StreamLine::Blocked { effect_id, sink })
        }
        "DENIED" => {
            let code = require_key(&map, "code")?.to_string();
            let sink = require_key(&map, "sink")?.to_string();
            Ok(StreamLine::Denied { code, sink })
        }
        "NODE_ALLOWED" => {
            let step = parse_usize(require_key(&map, "step")?, "step")?;
            let sink = require_key(&map, "sink")?.to_string();
            Ok(StreamLine::NodeAllowed { step, sink })
        }
        "STREAM_DONE" => {
            let submitted = parse_usize(require_key(&map, "submitted")?, "submitted")?;
            Ok(StreamLine::StreamDone { submitted })
        }
        other => Err(format!("unknown stream kind: {other}")),
    }
}

/// Parse a hold resume token. Exact trim match on `PROCEED` / `ABORT` only.
/// Unknown tokens are fail-closed errors — never treated as Proceed.
pub fn parse_hold_resume(line: &str) -> Result<HoldResume, String> {
    match line.trim() {
        "PROCEED" => Ok(HoldResume::Proceed),
        "ABORT" => Ok(HoldResume::Abort),
        other => Err(format!("unknown hold resume token: {other:?}")),
    }
}

fn require_key<'a>(map: &HashMap<&'a str, &'a str>, key: &str) -> Result<&'a str, String> {
    map.get(key)
        .copied()
        .ok_or_else(|| format!("missing required field `{key}`"))
}

fn parse_usize(raw: &str, field: &str) -> Result<usize, String> {
    raw.parse::<usize>()
        .map_err(|_| format!("invalid `{field}` value: {raw:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_parse_blocked_round_trip() {
        let line = StreamLine::Blocked {
            effect_id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into(),
            sink: "git.push".into(),
        };
        let s = format_line(&line);
        assert!(s.starts_with("caprun-stream: BLOCKED "));
        assert_eq!(parse_line(&s).unwrap(), line);
    }

    #[test]
    fn unknown_hold_token_is_not_proceed() {
        assert!(parse_hold_resume("YES").is_err());
        assert!(parse_hold_resume("proceed").is_err()); // case-sensitive
        assert_eq!(parse_hold_resume("  PROCEED\n").unwrap(), HoldResume::Proceed);
        assert_eq!(parse_hold_resume("ABORT").unwrap(), HoldResume::Abort);
    }

    #[test]
    fn map_stream_exit_taxonomy() {
        assert_eq!(
            map_stream_exit(StreamTerminalKind::Success).as_i32(),
            0
        );
        assert_eq!(
            map_stream_exit(StreamTerminalKind::DeniedAborted).as_i32(),
            2
        );
        assert_eq!(
            map_stream_exit(StreamTerminalKind::BlockedIncomplete).as_i32(),
            3
        );
        assert_eq!(
            map_stream_exit(StreamTerminalKind::InfraOrEmpty).as_i32(),
            1
        );
    }
}
