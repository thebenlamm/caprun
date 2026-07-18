//! policy_gate.rs (integration) — POLICY-01 distinctness + POLICY-02
//! enforcement-order proofs (DESIGN-v1.9-egress-policy §5.1/§5.2).
//!
//! These four proofs pin the load-bearing structural guarantees of Phase 42's
//! #1 adversarial-trace risk — the policy ↔ I2 boundary:
//!
//!   1. POLICY-01 distinctness — a policy-denied sink is a `Denied{PolicyDeny}`
//!      (`code()=="policy_deny"`), NOT a `BlockedPendingConfirmation`.
//!   2. POLICY-02 enforcement-order (the marquee proof) — a permissive
//!      (`allow_all`) policy that PERMITS `email.send` + `to` does NOT weaken the
//!      hardcoded I2 taint Block: a tainted routing-sensitive `to` STILL yields
//!      `BlockedPendingConfirmation`, byte-identical to the same input under a
//!      different permissive policy (`broker_default`). No policy, however
//!      permissive, removes an I2 Block (LOCKED; LIVE-06 leg 3 re-proves it live).
//!   3. Independence — a node that is BOTH policy-denied AND carries a tainted
//!      sensitive arg yields `Denied{PolicyDeny}` (the deny-only pre-gate fires
//!      first), so policy-deny and I2-Block are independently attributable.
//!   4. A policy-PERMITTED, clean (untainted, trusted-role) node yields `Allowed`.
//!
//! Links the executor with its `test-fixtures` feature (the existing
//! `crates/executor/tests/` convention) so `runtime_core::SessionPolicy` +
//! `test.observe` fixtures are available.

use executor::{submit_plan_node, value_store::ValueStore};
use runtime_core::plan_node::{PlanArg, PlanNode, SinkId, TaintLabel, ValueId};
use runtime_core::{DenyReason, ExecutorDecision, SessionPolicy, SessionStatus};
use uuid::Uuid;

/// Build an `email.send` plan node carrying a single `to` arg.
fn email_send_to(to: ValueId) -> PlanNode {
    PlanNode {
        sink: SinkId("email.send".to_string()),
        args: vec![PlanArg {
            name: "to".to_string(),
            value_id: to,
        }],
    }
}

/// Mint a tainted, routing-sensitive `to` value (role `email_address` so it
/// passes the Step-1c role gate and reaches the I2 sensitivity Block, not a
/// SlotTypeMismatch Deny) — the exact shape that Blocks under I2.
fn mint_tainted_recipient(store: &mut ValueStore) -> ValueId {
    store
        .mint(
            "accounts@ev1l.com".to_string(),
            vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw],
            vec![Uuid::new_v4()],
            Some("email_address".to_string()),
        )
        .expect("valid mint")
}

/// Mint a clean, trusted `to` value (role `recipient`, only `UserTrusted` taint)
/// — the shape that reaches `Allowed`.
fn mint_clean_recipient(store: &mut ValueStore) -> ValueId {
    store
        .mint(
            "boss@company.com".to_string(),
            vec![TaintLabel::UserTrusted],
            vec![Uuid::new_v4()],
            Some("recipient".to_string()),
        )
        .expect("valid mint")
}

/// POLICY-01: a plan node on a policy-DENIED sink returns `Denied{PolicyDeny}`
/// (`code()=="policy_deny"`) — a distinct Denied outcome, NEVER a
/// `BlockedPendingConfirmation`.
#[test]
fn policy_denied_sink_is_a_distinct_policy_deny_not_a_block() {
    let mut store = ValueStore::default();
    // A clean recipient — nothing here would ever I2-Block; the deny is purely
    // the policy refusing the sink.
    let to = mint_clean_recipient(&mut store);
    let plan = email_send_to(to);

    // `default_fail_closed()` lists NO sinks — email.send is not permitted.
    let policy = SessionPolicy::default_fail_closed();
    let decision = submit_plan_node(
        Uuid::new_v4(),
        Uuid::new_v4(),
        &plan,
        &store,
        &SessionStatus::Active,
        &policy,
    );

    match decision {
        ExecutorDecision::Denied { reason } => {
            assert_eq!(
                reason.code(),
                "policy_deny",
                "a policy refusal must carry the distinct policy_deny code"
            );
            assert_eq!(
                reason,
                DenyReason::PolicyDeny {
                    sink: "email.send".to_string(),
                    arg: None,
                    constraint: "sink-not-allowed".to_string(),
                },
                "a not-allowlisted sink denies sink-scoped (arg: None, sink-not-allowed)"
            );
        }
        other => panic!("expected Denied{{PolicyDeny}}, got {other:?}"),
    }
}

/// POLICY-02 (the marquee enforcement-order proof): with a permissive policy
/// that PERMITS `email.send` + `to`, a tainted routing-sensitive `to` STILL
/// returns `BlockedPendingConfirmation`. A permissive policy does NOT — and
/// cannot — remove the hardcoded I2 Block. Proven byte-identical across two
/// distinct permissive policies (`allow_all` and `broker_default`), so the
/// policy VALUE provably does not influence the I2 outcome.
#[test]
fn permissive_policy_does_not_weaken_the_i2_taint_block() {
    let mut store = ValueStore::default();
    let to = mint_tainted_recipient(&mut store);
    let plan = email_send_to(to);

    // Fixed effect_id so the two decisions are byte-comparable (the anchor
    // carries effect_id; a fresh uuid per call would differ spuriously).
    let session_id = Uuid::new_v4();
    let effect_id = Uuid::new_v4();

    let under_allow_all = submit_plan_node(
        session_id,
        effect_id,
        &plan,
        &store,
        &SessionStatus::Active,
        &SessionPolicy::allow_all(),
    );
    let under_broker_default = submit_plan_node(
        session_id,
        effect_id,
        &plan,
        &store,
        &SessionStatus::Active,
        &SessionPolicy::broker_default(),
    );

    // The permissive policy leaves the hardcoded I2 Block fully intact.
    assert!(
        matches!(
            under_allow_all,
            ExecutorDecision::BlockedPendingConfirmation { .. }
        ),
        "a tainted routing-sensitive `to` must still Block under a permissive \
         policy, got {under_allow_all:?}"
    );
    assert!(
        !matches!(under_allow_all, ExecutorDecision::Denied { .. }),
        "the permitted-but-tainted node must Block, never policy-Deny"
    );

    // Byte-identical across two different permissive policies: the policy value
    // provably does not change the I2 decision (POLICY-02).
    assert_eq!(
        under_allow_all, under_broker_default,
        "the I2 Block is byte-identical regardless of which permissive policy \
         permitted the sink — policy cannot influence the I2 outcome"
    );
}

/// Independence: a node that is BOTH policy-denied AND carries a tainted
/// sensitive arg returns `Denied{PolicyDeny}` — the deny-only pre-gate fires
/// BEFORE the I2 loop, so the two mechanisms stay independently attributable and
/// are never conflated.
#[test]
fn policy_deny_fires_before_i2_on_a_doubly_offending_node() {
    let mut store = ValueStore::default();
    // Tainted recipient: I2 WOULD Block this if the policy permitted the sink.
    let to = mint_tainted_recipient(&mut store);
    let plan = email_send_to(to);

    // But the policy denies the sink outright.
    let policy = SessionPolicy::default_fail_closed();
    let decision = submit_plan_node(
        Uuid::new_v4(),
        Uuid::new_v4(),
        &plan,
        &store,
        &SessionStatus::Active,
        &policy,
    );

    assert_eq!(
        decision,
        ExecutorDecision::Denied {
            reason: DenyReason::PolicyDeny {
                sink: "email.send".to_string(),
                arg: None,
                constraint: "sink-not-allowed".to_string(),
            }
        },
        "the deny-only pre-gate must fire first — a doubly-offending node is a \
         PolicyDeny, never an I2 Block"
    );
    assert!(
        !matches!(decision, ExecutorDecision::BlockedPendingConfirmation { .. }),
        "policy-deny and I2-Block must stay independently attributable"
    );
}

/// A policy-PERMITTED, clean (untainted, trusted-role) node returns `Allowed` —
/// policy permit + I2 pass.
#[test]
fn permitted_clean_node_is_allowed() {
    let mut store = ValueStore::default();
    let to = mint_clean_recipient(&mut store);
    let plan = email_send_to(to);

    let decision = submit_plan_node(
        Uuid::new_v4(),
        Uuid::new_v4(),
        &plan,
        &store,
        &SessionStatus::Active,
        &SessionPolicy::allow_all(),
    );

    assert_eq!(
        decision,
        ExecutorDecision::Allowed,
        "a policy-permitted, clean, trusted-role node must be Allowed"
    );
}
