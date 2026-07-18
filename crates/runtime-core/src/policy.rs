//! policy.rs — `SessionPolicy`, the hardcoded-schema pre-I2 narrowing gate.
//!
//! DESIGN-v1.9-egress-policy §5.1 (POLICY-01). A per-session policy is a
//! **minimal declarative, HARDCODED-schema** value (NOT Cedar, NO dynamic rule
//! engine) that specifies WHICH sinks are callable plus coarse arg constraints
//! (allowlisted hosts / paths / repos). It is a **pre-I2 narrowing gate**: it can
//! only *remove* authority (refuse a sink/arg that would otherwise be callable),
//! never *add* it.
//!
//! # The no-Allow structural pin (POLICY-02 pre-condition, §5.2 LOCKED)
//!
//! The evaluation surface returns ONLY permit-or-deny — `evaluate` returns
//! `Result<(), PolicyDenyKind>`: `Ok(())` means PERMIT (the executor falls
//! through to the UNMODIFIED I2 collect-then-Block loop), `Err(..)` means DENY.
//! There is NO variant, return value, or code path by which policy evaluation
//! produces an "Allow-and-skip" result — so no policy value, however permissive,
//! can weaken an I2 Block (T-42-01).
//!
//! # Purity (check-invariants Gate 2)
//!
//! runtime-core is I/O-forbidden. This file defines the policy TYPE and its
//! evaluation ONLY — no filesystem, network, or async tokens (check-invariants
//! Gate 2 greps for them). File reading + JSON parsing + the F1 containment
//! check happen in the brokerd binder (Plan 04), never here (T-42-03).

use std::collections::{BTreeMap, BTreeSet};

use crate::plan_node::SinkId;

/// The seven currently-callable production sinks (the `KNOWN_SINKS` surface in
/// `crates/executor/src/sink_schema.rs`). `broker_default()` allowlists exactly
/// these — an EXPLICIT allowlist, never allow-everything: a future/unknown sink
/// is NOT in this list and is therefore NOT callable under `broker_default()`.
const PRODUCTION_SINKS: &[&str] = &[
    "email.send",
    "file.create",
    "file.write",
    "process.exec",
    "git.commit",
    "http.request",
    "github.pr",
];

/// A coarse allowlist constraint on a single sink argument (POLICY-01).
///
/// Entries are matched as **prefixes**: a literal satisfies the constraint if it
/// equals or is prefixed by any allowlisted entry. This coarsely covers both the
/// host case (an allowlisted host is a prefix of the request `url`) and the path
/// case (an allowlisted path prefix of a `file.write` `path`). It is deliberately
/// minimal — the fine-grained F1 filesystem containment lives in adapter-fs
/// (Plan 02), never in this pure type.
///
/// An EMPTY allowlist denies every literal (fail-closed): a constrained arg with
/// no permitted prefixes admits nothing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ArgConstraint {
    /// Allowlisted literal values / prefixes (hosts, path prefixes, repos). A
    /// literal is permitted iff it `starts_with` one of these entries.
    pub allowed_prefixes: BTreeSet<String>,
}

impl ArgConstraint {
    /// Returns `true` iff `literal` matches (equals or is prefixed by) an
    /// allowlisted entry. An empty allowlist matches nothing (fail-closed).
    fn permits(&self, literal: &str) -> bool {
        self.allowed_prefixes
            .iter()
            .any(|prefix| literal.starts_with(prefix.as_str()))
    }
}

/// The machine-readable reason a policy evaluation DENIED a call.
///
/// This is the internal, policy.rs-local deny kind. The executor gate (Plan 03)
/// maps it onto the wire-crossing `DenyReason::PolicyDeny` variant
/// (executor_decision.rs). Kept as a small distinct type so both files stay
/// consistent and `evaluate` can return `Result<(), PolicyDenyKind>` with NO
/// Allow path (T-42-01). NOT `serde::Serialize`d itself — the wire type is
/// `DenyReason::PolicyDeny`, which carries owned `String` fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDenyKind {
    /// The target sink is not in `allowed_sinks` (deny-by-default sink gate).
    SinkNotAllowed,
    /// A constrained arg's literal did not match the coarse allowlist.
    ArgNotAllowlisted,
}

impl PolicyDenyKind {
    /// A short, stable machine-readable tag naming which policy rule refused.
    /// Feeds `DenyReason::PolicyDeny.constraint` in the executor gate (Plan 03).
    pub fn constraint_tag(&self) -> &'static str {
        match self {
            PolicyDenyKind::SinkNotAllowed => "sink-not-allowed",
            PolicyDenyKind::ArgNotAllowlisted => "arg-not-allowlisted",
        }
    }
}

/// A per-session policy: a hardcoded-schema, deny-by-default narrowing gate
/// (DESIGN §5.1, POLICY-01).
///
/// Deny-by-default on TWO axes:
///  1. **Sink gate** — a sink absent from `allowed_sinks` is NOT callable.
///  2. **Arg constraints** — a literal not matching a configured coarse allowlist
///     is refused. An arg with NO configured constraint is permitted once its
///     sink is allowed (arg constraints only *narrow* further; the sink gate is
///     the deny-by-default axis).
///
/// Serde-round-trips (loaded from a trusted JSON file by the brokerd binder in
/// Plan 04). `BTreeSet`/`BTreeMap` give a deterministic serialization order.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SessionPolicy {
    /// The deny-by-default sink gate: only sinks whose id string is in this set
    /// are callable at all.
    allowed_sinks: BTreeSet<String>,
    /// Coarse per-arg allowlists, keyed by sink id then arg name. An entry
    /// present here constrains that arg; an arg absent here is unconstrained
    /// (permitted once its sink is allowed).
    arg_constraints: BTreeMap<String, BTreeMap<String, ArgConstraint>>,
}

impl SessionPolicy {
    /// The empty, denies-EVERYTHING policy (deny-by-default / fail-closed). No
    /// sink is callable. This is the safe default a binder falls back to when it
    /// has no trusted policy to bind.
    pub fn default_fail_closed() -> Self {
        SessionPolicy {
            allowed_sinks: BTreeSet::new(),
            arg_constraints: BTreeMap::new(),
        }
    }

    /// Permits EVERY sink with no arg constraints. The permissive constructor
    /// Plan 03's POLICY-02 enforcement-order proof uses to show a permissive
    /// policy still cannot weaken an I2 Block, and the policy-agnostic default at
    /// existing (non-policy-specific) test call sites.
    ///
    /// NOTE: even this permissive policy has NO Allow-and-skip path — a permit
    /// merely hands the call to the UNMODIFIED I2 loop (§5.2 LOCKED).
    pub fn allow_all() -> Self {
        #[allow(unused_mut)]
        let mut allowed_sinks: BTreeSet<String> =
            PRODUCTION_SINKS.iter().map(|s| s.to_string()).collect();
        // Test-fixtures-gated: admit the `test.observe` fixture sink so the
        // policy-agnostic executor test call sites that pass `allow_all()` do
        // NOT PolicyDeny a `test.observe` plan node before it reaches I2. This
        // mirrors the IDENTICAL `#[cfg(any(test, feature = "test-fixtures"))]`
        // gate on `test.observe` in `crates/executor/src/{sink_sensitivity,
        // sink_schema}.rs`. NEVER present in a production build — `test-fixtures`
        // is never a default feature — so production `allow_all()` still lists
        // ONLY the seven real production sinks. `broker_default()` deliberately
        // does NOT get this gate (it is the production allowlist).
        #[cfg(any(test, feature = "test-fixtures"))]
        allowed_sinks.insert("test.observe".to_string());
        SessionPolicy {
            allowed_sinks,
            arg_constraints: BTreeMap::new(),
        }
    }

    /// An EXPLICIT deny-by-default allowlist of the seven currently-callable
    /// production sinks, with no arg constraints (DESIGN §5.1). This is NOT
    /// allow-everything: a future/unknown sink is NOT callable, preserving
    /// fail-closed while keeping existing end-to-end flows green. Plan 04's
    /// `bind_policy(None, ..)` returns this value — defining it here (wave 1)
    /// means Plan 04 never edits policy.rs.
    pub fn broker_default() -> Self {
        SessionPolicy {
            allowed_sinks: PRODUCTION_SINKS.iter().map(|s| s.to_string()).collect(),
            arg_constraints: BTreeMap::new(),
        }
    }

    /// Returns `true` iff `sink` is in the deny-by-default sink gate. The
    /// sink-level half of the narrowing gate the executor calls before the I2
    /// loop.
    pub fn permits_sink(&self, sink: &SinkId) -> bool {
        self.allowed_sinks.contains(&sink.0)
    }

    /// Evaluate ONE plan-node arg against the policy. Called once per arg by the
    /// executor gate (Plan 03).
    ///
    /// STRUCTURAL PIN (POLICY-02 pre-condition, §5.2): returns ONLY permit-or-deny.
    /// `Ok(())` = PERMIT (fall through to the unchanged I2 loop); `Err(kind)` =
    /// DENY. There is NO Allow-and-skip return.
    ///
    /// Deny-by-default:
    ///  - a sink not in `allowed_sinks` → `Err(SinkNotAllowed)`;
    ///  - a constrained arg whose literal matches no allowlist prefix →
    ///    `Err(ArgNotAllowlisted)`;
    ///  - an unconstrained arg on an allowed sink → `Ok(())`.
    pub fn evaluate(
        &self,
        sink: &SinkId,
        arg_name: &str,
        literal: &str,
    ) -> Result<(), PolicyDenyKind> {
        if !self.permits_sink(sink) {
            return Err(PolicyDenyKind::SinkNotAllowed);
        }
        if let Some(arg) = self
            .arg_constraints
            .get(&sink.0)
            .and_then(|args| args.get(arg_name))
        {
            if !arg.permits(literal) {
                return Err(PolicyDenyKind::ArgNotAllowlisted);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sink(name: &str) -> SinkId {
        SinkId(name.to_string())
    }

    #[test]
    fn empty_policy_denies_every_sink() {
        // A SessionPolicy that lists NO sinks denies every sink (deny-by-default).
        let policy = SessionPolicy::default_fail_closed();
        assert!(!policy.permits_sink(&sink("email.send")));
        assert_eq!(
            policy.evaluate(&sink("email.send"), "to", "a@b.com"),
            Err(PolicyDenyKind::SinkNotAllowed)
        );
    }

    #[test]
    fn allowed_sink_permits_that_sink_and_denies_another() {
        // A policy listing `email.send` permits email.send and denies git.commit.
        let mut allowed = BTreeSet::new();
        allowed.insert("email.send".to_string());
        let policy = SessionPolicy {
            allowed_sinks: allowed,
            arg_constraints: BTreeMap::new(),
        };
        assert!(policy.permits_sink(&sink("email.send")));
        assert_eq!(policy.evaluate(&sink("email.send"), "to", "a@b.com"), Ok(()));

        assert!(!policy.permits_sink(&sink("git.commit")));
        assert_eq!(
            policy.evaluate(&sink("git.commit"), "message", "x"),
            Err(PolicyDenyKind::SinkNotAllowed)
        );
    }

    #[test]
    fn coarse_arg_constraint_permits_matching_and_denies_non_matching() {
        // An allowlisted host prefix on an http.request `url` permits a literal
        // that satisfies the allowlist and denies one that does not.
        let mut allowed = BTreeSet::new();
        allowed.insert("http.request".to_string());
        let mut prefixes = BTreeSet::new();
        prefixes.insert("https://api.example.com".to_string());
        let mut url_c = BTreeMap::new();
        url_c.insert(
            "url".to_string(),
            ArgConstraint {
                allowed_prefixes: prefixes,
            },
        );
        let mut arg_constraints = BTreeMap::new();
        arg_constraints.insert("http.request".to_string(), url_c);
        let policy = SessionPolicy {
            allowed_sinks: allowed,
            arg_constraints,
        };

        assert_eq!(
            policy.evaluate(&sink("http.request"), "url", "https://api.example.com/v1/x"),
            Ok(())
        );
        assert_eq!(
            policy.evaluate(&sink("http.request"), "url", "https://evil.example.net/x"),
            Err(PolicyDenyKind::ArgNotAllowlisted)
        );
    }

    #[test]
    fn unconstrained_arg_on_allowed_sink_is_permitted() {
        // An arg with no configured constraint is permitted once its sink is
        // allowed (sink gate is the deny-by-default axis; arg constraints narrow).
        let mut allowed = BTreeSet::new();
        allowed.insert("http.request".to_string());
        let mut prefixes = BTreeSet::new();
        prefixes.insert("https://api.example.com".to_string());
        let mut url_c = BTreeMap::new();
        url_c.insert(
            "url".to_string(),
            ArgConstraint {
                allowed_prefixes: prefixes,
            },
        );
        let mut arg_constraints = BTreeMap::new();
        arg_constraints.insert("http.request".to_string(), url_c);
        let policy = SessionPolicy {
            allowed_sinks: allowed,
            arg_constraints,
        };

        // `method` has no configured constraint → permitted.
        assert_eq!(policy.evaluate(&sink("http.request"), "method", "GET"), Ok(()));
    }

    #[test]
    fn empty_arg_constraint_allowlist_denies_everything() {
        // A configured-but-empty allowlist admits nothing (fail-closed).
        let mut allowed = BTreeSet::new();
        allowed.insert("file.write".to_string());
        let mut path_c = BTreeMap::new();
        path_c.insert(
            "path".to_string(),
            ArgConstraint {
                allowed_prefixes: BTreeSet::new(),
            },
        );
        let mut arg_constraints = BTreeMap::new();
        arg_constraints.insert("file.write".to_string(), path_c);
        let policy = SessionPolicy {
            allowed_sinks: allowed,
            arg_constraints,
        };
        assert_eq!(
            policy.evaluate(&sink("file.write"), "path", "/ws/anything"),
            Err(PolicyDenyKind::ArgNotAllowlisted)
        );
    }

    #[test]
    fn serde_round_trips_through_json() {
        // SessionPolicy round-trips through serde from a JSON document (loaded
        // from a trusted file in Plan 04).
        let json = r#"{
            "allowed_sinks": ["email.send", "file.write"],
            "arg_constraints": {
                "file.write": {
                    "path": { "allowed_prefixes": ["/ws/out/"] }
                }
            }
        }"#;
        let parsed: SessionPolicy = serde_json::from_str(json).expect("parse");
        assert!(parsed.permits_sink(&sink("email.send")));
        assert_eq!(
            parsed.evaluate(&sink("file.write"), "path", "/ws/out/report.txt"),
            Ok(())
        );
        assert_eq!(
            parsed.evaluate(&sink("file.write"), "path", "/etc/passwd"),
            Err(PolicyDenyKind::ArgNotAllowlisted)
        );

        // Serialize → deserialize is identity.
        let s = serde_json::to_string(&parsed).expect("serialize");
        let reparsed: SessionPolicy = serde_json::from_str(&s).expect("reparse");
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn broker_default_permits_the_seven_production_sinks_and_denies_unlisted() {
        // broker_default() is an EXPLICIT allowlist, never allow-everything.
        let policy = SessionPolicy::broker_default();
        for s in [
            "email.send",
            "file.create",
            "file.write",
            "process.exec",
            "git.commit",
            "http.request",
            "github.pr",
        ] {
            assert!(policy.permits_sink(&sink(s)), "broker_default should permit {s}");
        }
        // A future/unknown sink is NOT callable.
        assert!(!policy.permits_sink(&sink("git.push")));
        assert_eq!(
            policy.evaluate(&sink("git.push"), "remote", "origin"),
            Err(PolicyDenyKind::SinkNotAllowed)
        );
    }

    #[test]
    fn allow_all_permits_production_sinks_with_no_arg_constraints() {
        let policy = SessionPolicy::allow_all();
        assert!(policy.permits_sink(&sink("email.send")));
        // No arg constraints → any literal on an allowed sink permits.
        assert_eq!(
            policy.evaluate(&sink("http.request"), "url", "https://anywhere.example/x"),
            Ok(())
        );
    }

    #[test]
    fn default_fail_closed_is_empty() {
        let policy = SessionPolicy::default_fail_closed();
        for s in PRODUCTION_SINKS {
            assert!(!policy.permits_sink(&sink(s)));
        }
    }

    #[test]
    fn policy_deny_kind_constraint_tags_are_stable() {
        assert_eq!(PolicyDenyKind::SinkNotAllowed.constraint_tag(), "sink-not-allowed");
        assert_eq!(
            PolicyDenyKind::ArgNotAllowlisted.constraint_tag(),
            "arg-not-allowlisted"
        );
    }
}
