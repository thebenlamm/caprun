/// executor_decision.rs — Integration tests for `submit_plan_node`.
///
/// Tests the four held behaviors from DESIGN-plan-executor §Executor Decision Logic:
///   1. Tainted routing-sensitive arg ("to") → BlockedPendingConfirmation carrying
///      literal/taint/provenance_chain verbatim from the ValueRecord.
///   2. Untainted routing-sensitive arg ("to") → Allowed.
///   3. Unknown/dangling handle → Denied (never Allowed — T-04-02).
///   4. Tainted content-sensitive arg ("subject") → Allowed in v0 (no Block;
///      verbatim Tier-4 review is deferred to the approval-hook plan).
///
/// Block payload fidelity requirement (plan §acceptance_criteria): the Block-case
/// assertion confirms literal_value, taint, and provenance_chain equal the values
/// passed to mint (not synthesized in the executor).

use executor::{submit_plan_node, value_store::ValueStore};
use sha2::{Digest, Sha256};
use runtime_core::{
    plan_node::{PlanArg, PlanNode, SinkId, TaintLabel, ValueId},
    ExecutorDecision,
};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn email_send_with_to(to_value_id: ValueId) -> PlanNode {
    PlanNode {
        sink: SinkId("email.send".to_string()),
        args: vec![PlanArg {
            name: "to".to_string(),
            value_id: to_value_id,
        }],
    }
}

fn email_send_with_arg(arg_name: &str, value_id: ValueId) -> PlanNode {
    PlanNode {
        sink: SinkId("email.send".to_string()),
        args: vec![PlanArg {
            name: arg_name.to_string(),
            value_id,
        }],
    }
}

// ---------------------------------------------------------------------------
// Case 1: tainted routing-sensitive arg → Block with verbatim record payload
// ---------------------------------------------------------------------------

/// A tainted value in the routing-sensitive "to" arg must produce
/// `BlockedPendingConfirmation` whose payload is sourced verbatim from the minted
/// ValueRecord — NOT synthesized by the executor.
///
/// Block payload fidelity: literal_value, taint, provenance_chain must EQUAL
/// the values passed to `mint` (no executor-side taint authoring, T-04-03).
#[test]
fn tainted_to_arg_blocks_with_verbatim_record_payload() {
    let mut store = ValueStore::default();
    let event_id = Uuid::new_v4();
    let provenance = vec![event_id];
    let literal = "accounts@ev1l.com".to_string();
    let taint = vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw];

    // mint: the broker's worker-extraction step
    let id = store
        .mint(literal.clone(), taint.clone(), provenance.clone())
        .expect("valid mint");

    let plan = email_send_with_to(id);
    let effect_id = Uuid::new_v4();
    let decision = submit_plan_node(Uuid::new_v4(), effect_id, &plan, &store);

    match decision {
        ExecutorDecision::BlockedPendingConfirmation { anchor, literal: block_literal } => {
            // Block payload fidelity (plan acceptance criteria) — every field is a
            // verbatim clone of the ValueRecord; nothing synthesized (T-04-03).
            // The LIVE literal rides on the decision (verbatim); the anchor carries
            // only its SHA-256 digest (redactable-at-rest, tamper-evident).
            assert_eq!(
                block_literal, literal,
                "decision.literal must be verbatim from ValueRecord, not synthesized"
            );
            let expected_digest = {
                let mut h = Sha256::new();
                h.update(literal.as_bytes());
                hex::encode(h.finalize())
            };
            assert_eq!(
                anchor.literal_sha256, expected_digest,
                "anchor.literal_sha256 must be sha256(literal) — tamper-evidence anchor"
            );
            assert_eq!(anchor.sink.0, "email.send");
            assert_eq!(anchor.arg, "to");
            assert_eq!(
                anchor.taint, taint,
                "taint must equal mint input — executor must not add/remove labels"
            );
            assert_eq!(
                anchor.provenance_chain, provenance,
                "provenance_chain[0] must equal the file_read Event id from mint"
            );
            // read_event_id is provenance_chain[0]; effect_id is the broker-supplied param.
            assert_eq!(anchor.read_event_id, provenance[0]);
            assert_eq!(anchor.effect_id, effect_id, "effect_id must be the passed-in param");
        }
        other => panic!("expected BlockedPendingConfirmation, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Case 2: untainted routing-sensitive arg → Allowed
// ---------------------------------------------------------------------------

/// A routing-sensitive "to" arg carrying ONLY trusted taint ([UserTrusted]) with
/// a real provenance anchor must produce `Allowed`. Empty taint is now forbidden at
/// the mint source (HARD-05) and would Deny at the executor's empty-taint guard, so
/// the clean allow-path is exercised with the real all-trusted shape.
#[test]
fn untainted_to_arg_returns_allowed() {
    let mut store = ValueStore::default();
    let event_id = Uuid::new_v4();
    // [UserTrusted] + real provenance anchor → clean, no untrusted label.
    let id = store
        .mint(
            "boss@company.com".to_string(),
            vec![TaintLabel::UserTrusted],
            vec![event_id],
        )
        .expect("valid mint");

    let plan = email_send_with_to(id);
    let decision = submit_plan_node(Uuid::new_v4(), Uuid::new_v4(), &plan, &store);

    assert_eq!(
        decision,
        ExecutorDecision::Allowed,
        "trusted-only 'to' must produce Allowed"
    );
}

// ---------------------------------------------------------------------------
// Case 3: unknown/dangling handle → Denied (T-04-02)
// ---------------------------------------------------------------------------

/// A `ValueId` not in the store resolves to `None`; the executor MUST return
/// `Denied`, never `Allowed`. This prevents an injected planner from fabricating
/// a handle to a non-existent clean value to bypass the Block check (T-04-02).
#[test]
fn unknown_handle_returns_denied() {
    let store = ValueStore::default(); // empty store
    let forged_id = ValueId::new(); // never minted

    let plan = email_send_with_to(forged_id);
    let decision = submit_plan_node(Uuid::new_v4(), Uuid::new_v4(), &plan, &store);

    assert!(
        matches!(decision, ExecutorDecision::Denied { .. }),
        "dangling handle must produce Denied, got {decision:?}"
    );
}

// ---------------------------------------------------------------------------
// Case 4: tainted content-sensitive arg → Allowed in v0
// ---------------------------------------------------------------------------

/// "subject" is content-sensitive, NOT routing-sensitive. A tainted subject does
/// NOT Block in v0 — Tier-4 verbatim review is the post-v0 approval mechanism.
/// This test guards against accidentally routing content-sensitive args through the
/// routing-sensitive Block path.
#[test]
fn tainted_content_sensitive_arg_allows_in_v0() {
    let mut store = ValueStore::default();
    let id = store
        .mint(
            "hostile subject line".to_string(),
            vec![TaintLabel::ExternalUntrusted],
            vec![Uuid::new_v4()],
        )
        .expect("valid mint");

    // "subject" is content-sensitive → must NOT Block in v0
    let plan = email_send_with_arg("subject", id);
    let decision = submit_plan_node(Uuid::new_v4(), Uuid::new_v4(), &plan, &store);

    assert_eq!(
        decision,
        ExecutorDecision::Allowed,
        "tainted 'subject' (content-sensitive) must produce Allowed in v0; \
         Tier-4 verbatim review is deferred to the approval-hook plan"
    );
}

// ---------------------------------------------------------------------------
// Additional coverage: cc/bcc routing-sensitive, body/attachment content-sensitive
// ---------------------------------------------------------------------------

/// "cc" and "bcc" are also routing-sensitive — tainted values must Block.
#[test]
fn tainted_cc_and_bcc_also_block() {
    let mut store = ValueStore::default();
    let cc_id = store
        .mint(
            "attacker@ev1l.com".to_string(),
            vec![TaintLabel::ExternalUntrusted],
            vec![Uuid::new_v4()],
        )
        .expect("valid mint");
    let bcc_id = store
        .mint(
            "spy@ev1l.com".to_string(),
            vec![TaintLabel::EmailRaw],
            vec![Uuid::new_v4()],
        )
        .expect("valid mint");

    for (arg_name, id) in [("cc", cc_id), ("bcc", bcc_id)] {
        let plan = email_send_with_arg(arg_name, id);
        let decision = submit_plan_node(Uuid::new_v4(), Uuid::new_v4(), &plan, &store);
        assert!(
            matches!(decision, ExecutorDecision::BlockedPendingConfirmation { .. }),
            "tainted '{arg_name}' must Block; got {decision:?}"
        );
    }
}

/// "body" and "attachment" are content-sensitive — tainted values must NOT Block.
#[test]
fn tainted_body_and_attachment_allow_in_v0() {
    let mut store = ValueStore::default();
    for arg_name in ["body", "attachment"] {
        let id = store
            .mint(
                format!("hostile {arg_name} content"),
                vec![TaintLabel::ExternalUntrusted],
                vec![Uuid::new_v4()],
            )
            .expect("valid mint");
        let plan = email_send_with_arg(arg_name, id);
        let decision = submit_plan_node(Uuid::new_v4(), Uuid::new_v4(), &plan, &store);
        assert_eq!(
            decision,
            ExecutorDecision::Allowed,
            "tainted '{arg_name}' (content-sensitive) must Allowed in v0; got {decision:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// HARD-02: predicate is over explicitly-untrusted labels only
// ---------------------------------------------------------------------------

/// HARD-02 allow case: a routing-sensitive "to" arg carrying ONLY [UserTrusted]
/// must produce `Allowed` — positive trusted provenance does NOT block.
///
/// Uses [UserTrusted] (NOT an empty vec!) so this test would still return
/// BlockedPendingConfirmation under the pre-fix predicate (`!record.taint.is_empty()`).
/// That property makes the fix provably load-bearing (Pitfall 2 in 06-RESEARCH.md).
#[test]
fn hard02_usertrusted_only_allows() {
    let mut store = ValueStore::default();
    let event_id = Uuid::new_v4();
    // Positive provenance: [UserTrusted] — must NOT block.
    let id = store
        .mint(
            "boss@company.com".to_string(),
            vec![TaintLabel::UserTrusted],
            vec![event_id],
        )
        .expect("valid mint");

    let plan = email_send_with_to(id);
    let decision = submit_plan_node(Uuid::new_v4(), Uuid::new_v4(), &plan, &store);

    assert_eq!(
        decision,
        ExecutorDecision::Allowed,
        "UserTrusted-only taint in routing-sensitive 'to' must produce Allowed (HARD-02)"
    );
}

/// HARD-02 regression guard: a routing-sensitive "to" arg with explicitly-untrusted
/// taint ([ExternalUntrusted, EmailRaw]) must still produce BlockedPendingConfirmation
/// after the predicate fix. Guards against accidentally opening the block path.
#[test]
fn hard02_externaltainted_still_blocks() {
    let mut store = ValueStore::default();
    let event_id = Uuid::new_v4();
    let literal = "attacker@evil.com".to_string();
    let taint = vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw];

    let id = store
        .mint(literal.clone(), taint.clone(), vec![event_id])
        .expect("valid mint");

    let plan = email_send_with_to(id);
    let decision = submit_plan_node(Uuid::new_v4(), Uuid::new_v4(), &plan, &store);

    assert!(
        matches!(decision, ExecutorDecision::BlockedPendingConfirmation { .. }),
        "ExternalUntrusted/EmailRaw taint in 'to' must still Block after HARD-02 predicate fix; got {decision:?}"
    );
}
