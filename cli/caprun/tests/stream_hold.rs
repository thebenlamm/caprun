//! stream_hold — Phase 50 Wave 0 protocol + exit-map unit tests (CLI-02 / CONFIRM-01)
//!
//! Host-safe pure tests for the parent↔worker line protocol and StreamExitCode
//! mapper. No broker, no confinement, no LIVE-07 claim.
//!
//! Product path is stay-connected parent-pipe only; reconnect-remint and
//! dual-Session stitch are rejected (DESIGN §3.3).

#[path = "../src/stream_hold.rs"]
mod stream_hold;

use runtime_core::executor_decision::DenyReason;
use stream_hold::{
    format_line, map_stream_exit, parse_hold_resume, parse_line, HoldResume, StreamExitCode,
    StreamLine, StreamTerminalKind,
};

#[test]
fn blocked_round_trip() {
    let line = StreamLine::Blocked {
        effect_id: "11111111-2222-3333-4444-555555555555".into(),
        sink: "git.push".into(),
    };
    let s = format_line(&line);
    assert_eq!(
        s,
        "caprun-stream: BLOCKED effect_id=11111111-2222-3333-4444-555555555555 sink=git.push"
    );
    assert_eq!(parse_line(&s).expect("parse BLOCKED"), line);
    // Whitespace-tolerant parse.
    assert_eq!(
        parse_line(&format!("  {s}  \n")).expect("trim parse"),
        line
    );
}

#[test]
fn denied_round_trip_including_policy_deny() {
    let line = StreamLine::Denied {
        code: "policy_deny".into(),
        sink: "file.create".into(),
    };
    let s = format_line(&line);
    assert_eq!(
        s,
        "caprun-stream: DENIED code=policy_deny sink=file.create"
    );
    assert_eq!(parse_line(&s).expect("parse DENIED"), line);

    // code= field equals DenyReason::PolicyDeny.code() — distinction from other
    // denies is the code field; exit integer shares 2 (RESEARCH A4).
    let policy = DenyReason::PolicyDeny {
        sink: "file.create".into(),
        arg: Some("path".into()),
        constraint: "test".into(),
    };
    assert_eq!(policy.code(), "policy_deny");
    assert_eq!(
        parse_line(&s).unwrap(),
        StreamLine::Denied {
            code: policy.code().into(),
            sink: "file.create".into(),
        }
    );
}

#[test]
fn stream_done_and_node_allowed_round_trip() {
    let done = StreamLine::StreamDone { submitted: 5 };
    let s = format_line(&done);
    assert_eq!(s, "caprun-stream: STREAM_DONE submitted=5");
    assert_eq!(parse_line(&s).unwrap(), done);

    let allowed = StreamLine::NodeAllowed {
        step: 2,
        sink: "git.commit".into(),
    };
    let s2 = format_line(&allowed);
    assert_eq!(s2, "caprun-stream: NODE_ALLOWED step=2 sink=git.commit");
    assert_eq!(parse_line(&s2).unwrap(), allowed);
}

#[test]
fn parse_fail_closed_on_garbage() {
    assert!(parse_line("hello world").is_err());
    assert!(parse_line("caprun-stream:").is_err());
    assert!(parse_line("caprun-stream: UNKNOWN x=1").is_err());
    assert!(parse_line("caprun-stream: BLOCKED sink=git.push").is_err()); // missing effect_id
    assert!(parse_line("caprun-stream: DENIED code=x").is_err()); // missing sink
    assert!(parse_line("caprun-stream: STREAM_DONE submitted=nope").is_err());
    assert!(parse_line("caprun-stream: BLOCKED effect_id=a not-a-kv").is_err());
}

#[test]
fn hold_resume_tokens_exact_match_only() {
    assert_eq!(parse_hold_resume("PROCEED").unwrap(), HoldResume::Proceed);
    assert_eq!(parse_hold_resume("ABORT").unwrap(), HoldResume::Abort);
    assert_eq!(
        parse_hold_resume("  PROCEED\n").unwrap(),
        HoldResume::Proceed
    );
    assert_eq!(parse_hold_resume("\tABORT  ").unwrap(), HoldResume::Abort);

    // Fail-closed: never Proceed on unknown / wrong case / free-form.
    for bad in [
        "",
        "proceed",
        "Proceed",
        "YES",
        "OK",
        "continue",
        "PROCEED extra",
        "ABORT\nPROCEED",
    ] {
        assert!(
            parse_hold_resume(bad).is_err(),
            "token {bad:?} must fail closed (not Proceed)"
        );
    }
}

#[test]
fn map_stream_exit_matrix_0_2_3_1() {
    assert_eq!(
        map_stream_exit(StreamTerminalKind::Success),
        StreamExitCode::Success
    );
    assert_eq!(map_stream_exit(StreamTerminalKind::Success).as_i32(), 0);

    assert_eq!(
        map_stream_exit(StreamTerminalKind::DeniedAborted),
        StreamExitCode::DeniedAborted
    );
    assert_eq!(
        map_stream_exit(StreamTerminalKind::DeniedAborted).as_i32(),
        2
    );

    assert_eq!(
        map_stream_exit(StreamTerminalKind::BlockedIncomplete),
        StreamExitCode::BlockedIncomplete
    );
    assert_eq!(
        map_stream_exit(StreamTerminalKind::BlockedIncomplete).as_i32(),
        3
    );

    assert_eq!(
        map_stream_exit(StreamTerminalKind::InfraOrEmpty),
        StreamExitCode::Infra
    );
    assert_eq!(
        map_stream_exit(StreamTerminalKind::InfraOrEmpty).as_i32(),
        1
    );

    // Empty stream is infra, not success (CLI-02).
    let empty = StreamTerminalKind::InfraOrEmpty;
    assert_ne!(map_stream_exit(empty).as_i32(), 0);
}

#[test]
fn i32_from_stream_exit_code() {
    assert_eq!(i32::from(StreamExitCode::Success), 0);
    assert_eq!(i32::from(StreamExitCode::Infra), 1);
    assert_eq!(i32::from(StreamExitCode::DeniedAborted), 2);
    assert_eq!(i32::from(StreamExitCode::BlockedIncomplete), 3);
}

/// Empty stream / hold-incomplete / deny share the pure mapper table —
/// empty is Infra=1 (never success); policy_deny and human ABORT share exit 2
/// with distinction only in the DENIED `code=` field (CLI-02).
#[test]
fn empty_and_policy_deny_exit_buckets() {
    assert_eq!(
        map_stream_exit(StreamTerminalKind::InfraOrEmpty).as_i32(),
        1,
        "empty stream is infra, not STREAM_DONE success"
    );
    // policy_deny is DeniedAborted bucket (exit 2), same as other denies.
    assert_eq!(
        map_stream_exit(StreamTerminalKind::DeniedAborted).as_i32(),
        2
    );
    let policy_line = format_line(&StreamLine::Denied {
        code: DenyReason::PolicyDeny {
            sink: "git.push".into(),
            arg: None,
            constraint: "not-allowlisted".into(),
        }
        .code()
        .into(),
        sink: "git.push".into(),
    });
    match parse_line(&policy_line).unwrap() {
        StreamLine::Denied { code, sink } => {
            assert_eq!(code, "policy_deny");
            assert_eq!(sink, "git.push");
        }
        other => panic!("expected DENIED, got {other:?}"),
    }
    // Parent must not treat BLOCKED as success (docs contract + parse shape).
    let blocked = format_line(&StreamLine::Blocked {
        effect_id: "00000000-0000-0000-0000-000000000001".into(),
        sink: "git.push".into(),
    });
    assert!(
        matches!(
            parse_line(&blocked).unwrap(),
            StreamLine::Blocked { .. }
        ),
        "BLOCKED is a hold signal, not success"
    );
    assert_ne!(
        map_stream_exit(StreamTerminalKind::BlockedIncomplete).as_i32(),
        0
    );
}
