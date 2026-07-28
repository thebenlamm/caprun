//! stream_substrate — Phase 48 STREAM-01/02 expansion proofs
//!
//! **Substrate proofs only** (worker sequential loop + opaque handle bag +
//! fail-closed mid-stream branches). This is **not** LIVE-07/08 CLI multi-step
//! DONE, **not** CaprunIntent coding recipe (Phase 49), and **not** Block-and-
//! Hold product UX (Phase 50). Hybrid in-crate multi-node is intentional for
//! the bag/taint spine.
//!
//! # Host-safe legs (always run)
//!
//! Pure planner/bag/decision-branch harnesses that mirror the production
//! branch table in `cli/caprun/src/worker.rs` (Allowed → bag any Some +
//! continue; BlockedPendingConfirmation → stop, no re-submit; Denied /
//! NotImplemented → abort remaining). Submit counts prove abort/stop without
//! a live broker.
//!
//! # Linux taint-via-bag leg (STREAM-02)
//!
//! `#[cfg(target_os = "linux")]` genuine exec-output mint → bag under
//! `out_0` → second `process.exec` command arg from bag → I2 Block with
//! provenance root on `process_exited` + `verify_chain` true. Modeled on
//! `s9_process_exec_block.rs` with bag intermediate storage (F-01 path).
//!
//! Linux verification (when Docker/mailpit available):
//!
//! ```text
//! MAILPIT_VERIFY_CMD='cargo build --workspace && cargo test -p caprun --test stream_substrate -- --nocapture' \
//!   bash scripts/mailpit-verify.sh
//! ```
//!
//! On macOS / non-Linux hosts, Linux-gated bodies compile away (0 passed for
//! those tests is expected per CLAUDE.md).

#[path = "../src/planner.rs"]
mod planner;

use runtime_core::{
    intent::CaprunIntent,
    plan_node::{PlanArg, PlanNode, SinkId, ValueId},
    ExecutorDecision,
};
use runtime_core::executor_decision::DenyReason;
use std::collections::HashMap;

// ── Production branch-table mirror (worker.rs sequential stream loop) ────────
//
// Keep these arms aligned with `cli/caprun/src/worker.rs` decision match:
//   Allowed → if Some(output_value_id) insert out_{step} (any sink, F-01); step++
//   BlockedPendingConfirmation → stop, no re-submit
//   Denied | NotImplemented → abort remaining

/// Terminal outcome of a driven stream (substrate, not product hold).
#[derive(Debug, Clone, PartialEq, Eq)]
enum StreamTerminal {
    /// At least one Allowed submit, then plan_next returned None.
    Success,
    /// BlockedPendingConfirmation — stop without re-submit (F-02).
    StopBlocked,
    /// Denied / NotImplemented — abort remaining (DESIGN §6.2).
    AbortDenied,
    /// plan_next returned None before any SubmitPlanNode (DESIGN §8.2).
    EmptyFailClosed,
}

/// One stream-loop branch after a PlanNodeDecision (mirrors worker).
#[derive(Debug, Clone, PartialEq, Eq)]
enum StreamBranch {
    Continue,
    StopBlocked,
    AbortDenied,
}

/// Apply one decision to the bag — exact F-01 / branch semantics of worker.rs.
fn apply_stream_decision(
    bag: &mut HashMap<String, ValueId>,
    step_index: usize,
    decision: &ExecutorDecision,
    output_value_id: Option<ValueId>,
) -> StreamBranch {
    match decision {
        ExecutorDecision::Allowed => {
            // F-01: store any Some(output_value_id) regardless of sink id.
            if let Some(id) = output_value_id {
                bag.insert(format!("out_{step_index}"), id);
            }
            StreamBranch::Continue
        }
        ExecutorDecision::BlockedPendingConfirmation { .. } => StreamBranch::StopBlocked,
        ExecutorDecision::Denied { .. } | ExecutorDecision::NotImplemented => {
            StreamBranch::AbortDenied
        }
    }
}

/// Drive sequential plan_next → submit → decision until terminal.
///
/// `decisions` is indexed by submit ordinal (0-based). Each entry is
/// `(ExecutorDecision, Option<output_value_id>)`. Returns
/// `(submit_count, submitted_plan_nodes, final_bag, terminal)`.
fn drive_stream<P: planner::Planner>(
    planner_impl: &P,
    intent: CaprunIntent,
    mut bag: HashMap<String, ValueId>,
    decisions: &[(ExecutorDecision, Option<ValueId>)],
) -> (
    usize,
    Vec<PlanNode>,
    HashMap<String, ValueId>,
    StreamTerminal,
) {
    let mut step_index: usize = 0;
    let mut submitted: usize = 0;
    let mut nodes: Vec<PlanNode> = Vec::new();

    loop {
        let ctx = planner::PlanStreamContext {
            intent: intent.clone(),
            step_index,
            handles: bag.clone(),
            task_instruction: None,
        };
        let Some(plan_node) = planner::Planner::plan_next(planner_impl, &ctx) else {
            break;
        };

        // Count SubmitPlanNode emissions (mock stream).
        submitted += 1;
        nodes.push(plan_node);

        let (decision, output_value_id) = decisions
            .get(submitted - 1)
            .cloned()
            .unwrap_or((ExecutorDecision::NotImplemented, None));

        match apply_stream_decision(&mut bag, step_index, &decision, output_value_id) {
            StreamBranch::Continue => {
                step_index += 1;
                continue;
            }
            StreamBranch::StopBlocked => {
                return (submitted, nodes, bag, StreamTerminal::StopBlocked);
            }
            StreamBranch::AbortDenied => {
                return (submitted, nodes, bag, StreamTerminal::AbortDenied);
            }
        }
    }

    if submitted == 0 {
        (0, nodes, bag, StreamTerminal::EmptyFailClosed)
    } else {
        (submitted, nodes, bag, StreamTerminal::Success)
    }
}

fn seed_bag(intent: ValueId) -> HashMap<String, ValueId> {
    let mut bag = HashMap::new();
    bag.insert("intent".into(), intent.clone());
    bag.insert("trusted_subject".into(), intent.clone());
    bag.insert("trusted_body".into(), intent);
    bag
}

fn file_intent(path: &str) -> CaprunIntent {
    CaprunIntent::CreateFileFromReport {
        path: path.into(),
    }
}

fn denied_policy() -> ExecutorDecision {
    ExecutorDecision::Denied {
        reason: DenyReason::PolicyDeny {
            sink: "file.create".into(),
            arg: Some("path".into()),
            constraint: "test-deny-abort".into(),
        },
    }
}

fn blocked_pending() -> ExecutorDecision {
    ExecutorDecision::BlockedPendingConfirmation { anchors: vec![] }
}

// ── Test-only multi-node planners (no CaprunIntent coding variant) ───────────

/// Emits up to `n` trivial file.create nodes (n ∈ 0..=3 for fixtures).
struct NNodePlanner {
    n: usize,
}

impl planner::Planner for NNodePlanner {
    fn plan(
        &self,
        _intent: &CaprunIntent,
        _intent_value_id: ValueId,
        _derived_recipient: Option<ValueId>,
        _body: Option<ValueId>,
        _trusted_subject_handle: ValueId,
        _trusted_body_handle: ValueId,
        _task_instruction: Option<String>,
    ) -> PlanNode {
        unreachable!("NNodePlanner uses plan_next only")
    }

    fn plan_next(&self, ctx: &planner::PlanStreamContext) -> Option<PlanNode> {
        if ctx.step_index >= self.n {
            return None;
        }
        let path = ctx.handles.get("intent")?.clone();
        Some(PlanNode {
            sink: SinkId("file.create".into()),
            args: vec![
                PlanArg {
                    name: "path".into(),
                    value_id: path.clone(),
                },
                PlanArg {
                    name: "contents".into(),
                    value_id: path,
                },
            ],
        })
    }
}

/// 3-node planner: step 1 places bag `out_0` into path when present (STREAM-02
/// placement adjacency for deny/block sequencing).
struct ThreeNodeBagPlanner;

impl planner::Planner for ThreeNodeBagPlanner {
    fn plan(
        &self,
        _intent: &CaprunIntent,
        _intent_value_id: ValueId,
        _derived_recipient: Option<ValueId>,
        _body: Option<ValueId>,
        _trusted_subject_handle: ValueId,
        _trusted_body_handle: ValueId,
        _task_instruction: Option<String>,
    ) -> PlanNode {
        unreachable!("ThreeNodeBagPlanner uses plan_next only")
    }

    fn plan_next(&self, ctx: &planner::PlanStreamContext) -> Option<PlanNode> {
        match ctx.step_index {
            0 | 2 => {
                let path = ctx.handles.get("intent")?.clone();
                Some(PlanNode {
                    sink: SinkId("file.create".into()),
                    args: vec![
                        PlanArg {
                            name: "path".into(),
                            value_id: path.clone(),
                        },
                        PlanArg {
                            name: "contents".into(),
                            value_id: path,
                        },
                    ],
                })
            }
            1 => {
                let bag_or_intent = ctx
                    .handles
                    .get("out_0")
                    .or_else(|| ctx.handles.get("intent"))?
                    .clone();
                let contents = ctx.handles.get("intent")?.clone();
                Some(PlanNode {
                    sink: SinkId("file.create".into()),
                    args: vec![
                        PlanArg {
                            name: "path".into(),
                            value_id: bag_or_intent,
                        },
                        PlanArg {
                            name: "contents".into(),
                            value_id: contents,
                        },
                    ],
                })
            }
            _ => None,
        }
    }
}

// ── Host-safe STREAM expansion tests ─────────────────────────────────────────

/// F-01 unit: bag insert has no process.exec-only filter — any Some(output_value_id)
/// for multiple sink families is stored under `out_{step}`.
#[test]
fn f01_bag_stores_any_some_output_value_id_multi_sink() {
    let mut bag = seed_bag(ValueId::new());

    // Simulated Allowed decisions with Some for multi-sink mint arms (authority:
    // process.exec / git.commit / http.request) plus a non-mint sink that still
    // must store if Some were ever returned (no sink-id filter).
    let cases: &[(usize, &str)] = &[
        (0, "process.exec"),
        (1, "git.commit"),
        (2, "http.request"),
        (3, "file.create"),
    ];
    let mut expected: Vec<(String, ValueId)> = Vec::new();
    for &(step, _sink) in cases {
        let id = ValueId::new();
        let branch = apply_stream_decision(
            &mut bag,
            step,
            &ExecutorDecision::Allowed,
            Some(id.clone()),
        );
        assert_eq!(branch, StreamBranch::Continue);
        expected.push((format!("out_{step}"), id));
    }

    for (key, id) in &expected {
        assert_eq!(
            bag.get(key),
            Some(id),
            "F-01: bag must store Some(output_value_id) under {key} for any sink"
        );
    }

    // Allowed + None must not invent an out_ key.
    let before = bag.len();
    let branch = apply_stream_decision(&mut bag, 99, &ExecutorDecision::Allowed, None);
    assert_eq!(branch, StreamBranch::Continue);
    assert_eq!(bag.len(), before);
    assert!(!bag.contains_key("out_99"));
}

/// Deny aborts remaining: a 3-node planner that would emit 3 nodes; after node 2
/// returns Denied, no third SubmitPlanNode is issued (submit count == 2).
#[test]
fn deny_aborts_remaining_no_further_submit() {
    let intent_vid = ValueId::new();
    let bag = seed_bag(intent_vid);
    let planner_impl = ThreeNodeBagPlanner;
    let out0 = ValueId::new();

    let decisions = [
        (ExecutorDecision::Allowed, Some(out0.clone())),
        (denied_policy(), None),
        // Would be node 3 — must never be consumed.
        (ExecutorDecision::Allowed, None),
    ];

    let (submitted, nodes, final_bag, terminal) = drive_stream(
        &planner_impl,
        file_intent("deny-abort.txt"),
        bag,
        &decisions,
    );

    assert_eq!(
        submitted, 2,
        "after Denied on node 2, submit counter must stay at 2 (no third SubmitPlanNode)"
    );
    assert_eq!(nodes.len(), 2);
    assert_eq!(terminal, StreamTerminal::AbortDenied);
    // First Allowed still bagged out_0 (F-01 path before deny).
    assert_eq!(final_bag.get("out_0"), Some(&out0));
    // Node 2 path should have used bagged out_0 (STREAM-02 placement).
    assert_eq!(
        nodes[1]
            .args
            .iter()
            .find(|a| a.name == "path")
            .map(|a| &a.value_id),
        Some(&out0),
        "step 1 must place bagged out_0 before the deny decision is applied"
    );
}

/// policy_deny / Denied mid-stream is the same abort-remaining branch as
/// structural Denied (DESIGN §6.1–§6.2).
#[test]
fn not_implemented_also_aborts_remaining() {
    let planner_impl = NNodePlanner { n: 3 };
    let decisions = [
        (ExecutorDecision::Allowed, None),
        (ExecutorDecision::NotImplemented, None),
        (ExecutorDecision::Allowed, None),
    ];
    let (submitted, _, _, terminal) = drive_stream(
        &planner_impl,
        file_intent("ni.txt"),
        seed_bag(ValueId::new()),
        &decisions,
    );
    assert_eq!(submitted, 2);
    assert_eq!(terminal, StreamTerminal::AbortDenied);
}

/// Block no re-submit: after BlockedPendingConfirmation on node k, the blocked
/// plan_node is not submitted again; loop stops fail-closed (F-02 / DESIGN §2.2).
#[test]
fn block_stops_without_resubmit() {
    let planner_impl = NNodePlanner { n: 3 };
    let blocked_node_fingerprint = {
        // Capture what step 0 would emit so we can prove it is not re-submitted.
        let intent_vid = ValueId::new();
        let bag = seed_bag(intent_vid.clone());
        let ctx = planner::PlanStreamContext {
            intent: file_intent("block.txt"),
            step_index: 0,
            handles: bag,
            task_instruction: None,
        };
        planner::Planner::plan_next(&planner_impl, &ctx).expect("step 0 node")
    };

    let decisions = [
        (blocked_pending(), None),
        // Must never be reached / submitted.
        (ExecutorDecision::Allowed, None),
        (ExecutorDecision::Allowed, None),
    ];

    let (submitted, nodes, _, terminal) = drive_stream(
        &planner_impl,
        file_intent("block.txt"),
        seed_bag(ValueId::new()),
        &decisions,
    );

    assert_eq!(
        submitted, 1,
        "Block must stop the stream: exactly one SubmitPlanNode (the blocked node)"
    );
    assert_eq!(nodes.len(), 1);
    assert_eq!(terminal, StreamTerminal::StopBlocked);
    // The single submission is the blocked node — not a second attempt.
    assert_eq!(nodes[0].sink, blocked_node_fingerprint.sink);
    assert_eq!(nodes[0].args.len(), blocked_node_fingerprint.args.len());

    // Drive again with Block on node 2 after one Allowed — still no re-submit of
    // the blocked node (submit stays at the blocked step index + 1).
    let out0 = ValueId::new();
    let decisions2 = [
        (ExecutorDecision::Allowed, Some(out0)),
        (blocked_pending(), None),
        (ExecutorDecision::Allowed, None),
    ];
    let (submitted2, nodes2, _, terminal2) = drive_stream(
        &ThreeNodeBagPlanner,
        file_intent("block2.txt"),
        seed_bag(ValueId::new()),
        &decisions2,
    );
    assert_eq!(submitted2, 2, "Block on node 2 → submit count == 2, not 3");
    assert_eq!(nodes2.len(), 2);
    assert_eq!(terminal2, StreamTerminal::StopBlocked);
    // No third node — blocked node not re-submitted as a third emission.
}

/// Empty stream still fails closed (Plan 01 regression / DESIGN §8.2).
#[test]
fn empty_stream_fails_closed() {
    let planner_impl = NNodePlanner { n: 0 };
    let (submitted, nodes, _, terminal) = drive_stream(
        &planner_impl,
        file_intent("empty.txt"),
        seed_bag(ValueId::new()),
        &[],
    );
    assert_eq!(submitted, 0);
    assert!(nodes.is_empty());
    assert_eq!(terminal, StreamTerminal::EmptyFailClosed);
}

/// Single Allowed node then None is the success path.
#[test]
fn single_allowed_node_then_none_is_success() {
    let planner_impl = NNodePlanner { n: 1 };
    let (submitted, nodes, bag, terminal) = drive_stream(
        &planner_impl,
        file_intent("single.txt"),
        seed_bag(ValueId::new()),
        &[(ExecutorDecision::Allowed, None)],
    );
    assert_eq!(submitted, 1);
    assert_eq!(nodes.len(), 1);
    assert_eq!(terminal, StreamTerminal::Success);
    assert!(!bag.contains_key("out_0"), "None output leaves bag without out_0");
}

/// Full Allowed multi-node stream bags every Some and ends Success.
#[test]
fn multi_allowed_bags_each_some_and_succeeds() {
    let planner_impl = ThreeNodeBagPlanner;
    let id0 = ValueId::new();
    let id1 = ValueId::new();
    let id2 = ValueId::new();
    let decisions = [
        (ExecutorDecision::Allowed, Some(id0.clone())),
        (ExecutorDecision::Allowed, Some(id1.clone())),
        (ExecutorDecision::Allowed, Some(id2.clone())),
    ];
    let (submitted, _, bag, terminal) = drive_stream(
        &planner_impl,
        file_intent("multi-ok.txt"),
        seed_bag(ValueId::new()),
        &decisions,
    );
    assert_eq!(submitted, 3);
    assert_eq!(terminal, StreamTerminal::Success);
    assert_eq!(bag.get("out_0"), Some(&id0));
    assert_eq!(bag.get("out_1"), Some(&id1));
    assert_eq!(bag.get("out_2"), Some(&id2));
}

// ── Linux taint-via-bag (STREAM-02) — Task 2 extends this module ─────────────
// Placeholder: genuine bag-taint leg lands in Plan 48-02 Task 2.
// #[cfg(target_os = "linux")] mod linux { ... }

/// Always-on guard so the test binary is never empty on non-Linux hosts.
#[test]
fn stream_substrate_host_guard_compiles() {
    assert!(
        matches!(
            apply_stream_decision(
                &mut HashMap::new(),
                0,
                &ExecutorDecision::Allowed,
                None
            ),
            StreamBranch::Continue
        )
    );
}
