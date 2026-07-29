//! stream_substrate — Phase 48 STREAM-01/02 + Phase 50 HoldContinue/HoldAbort
//!
//! **Substrate proofs** (worker sequential loop + opaque handle bag +
//! fail-closed mid-stream branches + Phase 50 Block-and-Hold branch table).
//! This is **not** LIVE-07/08 CLI multi-step DONE. Hybrid in-crate multi-node
//! is intentional for the bag/taint spine; hold proofs model PROCEED/ABORT
//! without a live broker.
//!
//! # Host-safe legs (always run)
//!
//! Pure planner/bag/decision-branch harnesses that mirror the production
//! branch table in `cli/caprun/src/worker.rs` (Allowed → bag any Some +
//! continue; BlockedPendingConfirmation → stop or hold; Denied /
//! NotImplemented → abort remaining). Submit counts prove abort/stop/hold
//! without a live broker.
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
//   BlockedPendingConfirmation → stop (email/file) OR hold (coding)
//   Denied | NotImplemented → abort remaining
//
// Hold path (Phase 50 CONFIRM-01): on Block, optional hold action Proceed
// advances step_index without counting a second submit of the blocked node;
// Abort returns AbortDenied without further submits.

/// Terminal outcome of a driven stream.
#[derive(Debug, Clone, PartialEq, Eq)]
enum StreamTerminal {
    /// At least one Allowed submit, then plan_next returned None.
    Success,
    /// BlockedPendingConfirmation — stop without re-submit (F-02; non-hold).
    StopBlocked,
    /// Denied / NotImplemented / hold ABORT — abort remaining (DESIGN §6.2).
    AbortDenied,
    /// plan_next returned None before any SubmitPlanNode (DESIGN §8.2).
    EmptyFailClosed,
}

/// One stream-loop branch after a PlanNodeDecision (mirrors worker).
#[derive(Debug, Clone, PartialEq, Eq)]
enum StreamBranch {
    Continue,
    /// Non-hold stop (email/file single-node Block → exit 3).
    StopBlocked,
    AbortDenied,
    /// Coding hold: Block signal; caller must supply Proceed/Abort.
    Hold,
}

/// Hold resume outcome supplied by the parent (mirrors PROCEED/ABORT tokens).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HoldAction {
    /// Advance step_index without re-submit (confirm already ran the sink).
    Proceed,
    /// Abort remaining — terminal AbortDenied, exit 2.
    Abort,
}

/// Apply one decision to the bag — exact F-01 / branch semantics of worker.rs.
///
/// On Block this returns `Hold` when `hold_enabled` (coding path) so the
/// driver can apply Proceed/Abort; otherwise `StopBlocked` (email/file).
fn apply_stream_decision(
    bag: &mut HashMap<String, ValueId>,
    step_index: usize,
    decision: &ExecutorDecision,
    output_value_id: Option<ValueId>,
) -> StreamBranch {
    apply_stream_decision_ex(bag, step_index, decision, output_value_id, false)
}

fn apply_stream_decision_ex(
    bag: &mut HashMap<String, ValueId>,
    step_index: usize,
    decision: &ExecutorDecision,
    output_value_id: Option<ValueId>,
    hold_enabled: bool,
) -> StreamBranch {
    match decision {
        ExecutorDecision::Allowed => {
            // F-01: store any Some(output_value_id) regardless of sink id.
            if let Some(id) = output_value_id {
                bag.insert(format!("out_{step_index}"), id);
            }
            StreamBranch::Continue
        }
        ExecutorDecision::BlockedPendingConfirmation { .. } => {
            if hold_enabled {
                StreamBranch::Hold
            } else {
                StreamBranch::StopBlocked
            }
        }
        ExecutorDecision::Denied { .. } | ExecutorDecision::NotImplemented => {
            StreamBranch::AbortDenied
        }
    }
}

/// Drive sequential plan_next → submit → decision until terminal (non-hold).
///
/// `decisions` is indexed by submit ordinal (0-based). Each entry is
/// `(ExecutorDecision, Option<output_value_id>)`. Returns
/// `(submit_count, submitted_plan_nodes, final_bag, terminal)`.
fn drive_stream<P: planner::Planner>(
    planner_impl: &P,
    intent: CaprunIntent,
    bag: HashMap<String, ValueId>,
    decisions: &[(ExecutorDecision, Option<ValueId>)],
) -> (
    usize,
    Vec<PlanNode>,
    HashMap<String, ValueId>,
    StreamTerminal,
) {
    drive_stream_with_hold(planner_impl, intent, bag, decisions, &[])
}

/// Drive sequential plan_next → submit → decision with optional hold actions.
///
/// When a Block decision is encountered and a next `HoldAction` is available,
/// Proceed advances `step_index` without re-submitting the blocked node;
/// Abort returns AbortDenied without further submits. If no hold action
/// remains, Block maps to StopBlocked (non-hold substrate path).
///
/// `hold_actions` is consumed in order for each Block encountered.
fn drive_stream_with_hold<P: planner::Planner>(
    planner_impl: &P,
    intent: CaprunIntent,
    mut bag: HashMap<String, ValueId>,
    decisions: &[(ExecutorDecision, Option<ValueId>)],
    hold_actions: &[HoldAction],
) -> (
    usize,
    Vec<PlanNode>,
    HashMap<String, ValueId>,
    StreamTerminal,
) {
    let mut step_index: usize = 0;
    let mut submitted: usize = 0;
    let mut nodes: Vec<PlanNode> = Vec::new();
    let mut hold_idx: usize = 0;
    // Hold is enabled when the caller supplied any hold actions (coding path
    // simulation) OR when intent is SafeCodingWorkflow — mirrors worker gate.
    let hold_enabled = !hold_actions.is_empty()
        || matches!(intent, CaprunIntent::SafeCodingWorkflow { .. });

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

        match apply_stream_decision_ex(
            &mut bag,
            step_index,
            &decision,
            output_value_id,
            hold_enabled,
        ) {
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
            StreamBranch::Hold => {
                // No re-submit of the blocked node. Apply parent hold action.
                let action = hold_actions.get(hold_idx).copied();
                hold_idx += 1;
                match action {
                    Some(HoldAction::Proceed) => {
                        // Confirm already executed sink from durable snapshot.
                        step_index += 1;
                        continue;
                    }
                    Some(HoldAction::Abort) => {
                        return (submitted, nodes, bag, StreamTerminal::AbortDenied);
                    }
                    None => {
                        // Hold enabled but no action supplied → incomplete hold.
                        return (submitted, nodes, bag, StreamTerminal::StopBlocked);
                    }
                }
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

// ── Phase 50 HoldContinue / HoldAbort (CONFIRM-01) ───────────────────────────

/// Five-node coding recipe mirror: file.write → process.exec → git.commit →
/// git.push → github.pr (same sink order as `plan_coding_next`).
struct CodingFiveNodePlanner;

impl planner::Planner for CodingFiveNodePlanner {
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
        unreachable!("CodingFiveNodePlanner uses plan_next only")
    }

    fn plan_next(&self, ctx: &planner::PlanStreamContext) -> Option<PlanNode> {
        let sink = match ctx.step_index {
            0 => "file.write",
            1 => "process.exec",
            2 => "git.commit",
            3 => "git.push",
            4 => "github.pr",
            _ => return None,
        };
        let handle = ctx.handles.get("intent")?.clone();
        Some(PlanNode {
            sink: SinkId(sink.into()),
            args: vec![PlanArg {
                name: "arg".into(),
                value_id: handle,
            }],
        })
    }
}

fn coding_intent() -> CaprunIntent {
    CaprunIntent::SafeCodingWorkflow {
        path: "src/main.rs".into(),
        contents: "fn main() {}".into(),
        test_command: "/bin/true".into(),
        test_args_json: "[]".into(),
        commit_message: "wip".into(),
        remote: "origin".into(),
        refspec: "HEAD:refs/heads/feat".into(),
        owner: "acme".into(),
        repo: "demo".into(),
        base: "main".into(),
        head: "feat".into(),
        pr_title: "feat".into(),
        pr_body: "body".into(),
    }
}

/// HoldContinue: Allowed×3, Block(push), Proceed, Allowed(pr) — submit count
/// grows only for subsequent sinks; git.push appears exactly once (no re-submit).
#[test]
fn hold_continue_no_resubmit_blocked_sink() {
    let planner_impl = CodingFiveNodePlanner;
    let decisions = [
        (ExecutorDecision::Allowed, None), // file.write
        (ExecutorDecision::Allowed, None), // process.exec
        (ExecutorDecision::Allowed, None), // git.commit
        (blocked_pending(), None),         // git.push — always-confirm Block
        (ExecutorDecision::Allowed, None), // github.pr after PROCEED
    ];
    let hold = [HoldAction::Proceed];

    let (submitted, nodes, _, terminal) = drive_stream_with_hold(
        &planner_impl,
        coding_intent(),
        seed_bag(ValueId::new()),
        &decisions,
        &hold,
    );

    assert_eq!(
        terminal,
        StreamTerminal::Success,
        "PROCEED after push Block must reach STREAM success with remaining Allowed"
    );
    assert_eq!(
        submitted, 5,
        "submit count must be 5 (push once + pr after hold), not 6"
    );
    assert_eq!(nodes.len(), 5);

    let sinks: Vec<&str> = nodes.iter().map(|n| n.sink.0.as_str()).collect();
    assert_eq!(
        sinks,
        vec![
            "file.write",
            "process.exec",
            "git.commit",
            "git.push",
            "github.pr",
        ],
        "sink order must match coding recipe; no re-submit of blocked node"
    );
    let push_count = sinks.iter().filter(|s| **s == "git.push").count();
    assert_eq!(
        push_count, 1,
        "git.push must appear exactly once after PROCEED (no second SubmitPlanNode)"
    );
}

/// HoldAbort: after Block, Abort → no further plan_next submits; terminal denied.
#[test]
fn hold_abort_stops_without_further_submits() {
    let planner_impl = CodingFiveNodePlanner;
    let decisions = [
        (ExecutorDecision::Allowed, None), // file.write
        (blocked_pending(), None),         // process.exec Block mid-stream
        (ExecutorDecision::Allowed, None), // must never be submitted
        (ExecutorDecision::Allowed, None),
        (ExecutorDecision::Allowed, None),
    ];
    let hold = [HoldAction::Abort];

    let (submitted, nodes, _, terminal) = drive_stream_with_hold(
        &planner_impl,
        coding_intent(),
        seed_bag(ValueId::new()),
        &decisions,
        &hold,
    );

    assert_eq!(terminal, StreamTerminal::AbortDenied);
    assert_eq!(
        submitted, 2,
        "ABORT after Block must not submit any later sinks (submitted stays at blocked node)"
    );
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[1].sink.0, "process.exec");
    // No git.commit / git.push / github.pr after abort.
    assert!(nodes.iter().all(|n| n.sink.0 != "git.commit"));
    assert!(nodes.iter().all(|n| n.sink.0 != "github.pr"));
}

/// Silent continue-past-Block is impossible: Block without hold action stops
/// (does not return Success).
#[test]
fn block_without_proceed_is_not_success() {
    let planner_impl = CodingFiveNodePlanner;
    let decisions = [
        (ExecutorDecision::Allowed, None),
        (blocked_pending(), None),
        (ExecutorDecision::Allowed, None),
    ];
    // hold_enabled via SafeCodingWorkflow intent, but empty hold_actions → incomplete.
    let (submitted, _, _, terminal) = drive_stream_with_hold(
        &planner_impl,
        coding_intent(),
        seed_bag(ValueId::new()),
        &decisions,
        &[], // no PROCEED
    );
    assert_eq!(submitted, 2);
    assert_ne!(terminal, StreamTerminal::Success);
    assert_eq!(terminal, StreamTerminal::StopBlocked);
}

// ── Linux taint-via-bag (STREAM-02) ──────────────────────────────────────────
//
// Substrate proof: bag intermediate storage of a genuine mint_from_exec handle
// then plan_next-style placement into process.exec/command → I2 Block.
// Hybrid in-crate multi-node — **not** LIVE-07 CLI multi-step DONE.

#[cfg(target_os = "linux")]
mod linux {
    use super::{apply_stream_decision, StreamBranch};
    use adapter_fs::workspace::WorkspaceRoot;
    use brokerd::audit::{
        append_event, find_event_by_type, open_audit_db, verify_chain,
    };
    use brokerd::quarantine::mint_from_exec;
    use brokerd::sinks::process_exec::invoke_process_exec;
    use chrono::Utc;
    use executor::value_store::ValueStore;
    use runtime_core::plan_node::{PlanArg, SinkId, TaintLabel, ValueId};
    use runtime_core::{Event, ExecutorDecision, PlanNode, SessionStatus};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    /// Fixed, non-secret test MAC key (mirrors s9_process_exec_block).
    const TEST_KEY: &[u8] = b"stream-substrate-taint-via-bag-test-key";

    fn mint_trusted(store: &mut ValueStore, literal: &str) -> ValueId {
        store
            .mint(
                literal.to_string(),
                vec![TaintLabel::UserTrusted],
                vec![Uuid::new_v4()],
                None,
            )
            .expect("mint trusted literal")
    }

    fn seed_root_event(conn: &rusqlite::Connection, session_id: Uuid) -> (Uuid, String) {
        let root = Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "broker".into(),
            "session_created".into(),
            Utc::now(),
            vec![],
        );
        let hash = append_event(conn, TEST_KEY, &root, None).expect("append root event");
        (root.id, hash)
    }

    fn fresh_workspace(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "caprun_stream_substrate_{tag}_{}",
            Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create workspace dir");
        dir
    }

    /// STREAM-02 genuine taint via bag: Node1 trusted process.exec Allowed →
    /// mint_from_exec → bag under `out_0` (F-01 path) → Node2 process.exec
    /// command = bag handle → BlockedPendingConfirmation with provenance root
    /// on process_exited + verify_chain true. No effect of the blocked node.
    #[tokio::test]
    async fn taint_via_bag_exec_output_blocks_with_genuine_provenance() {
        let conn = Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));
        let session_id = Uuid::new_v4();
        let (root_id, root_hash) = {
            let locked = conn.lock().expect("lock conn");
            seed_root_event(&locked, session_id)
        };

        let mut store = ValueStore::default();
        let command_vid = mint_trusted(&mut store, "/bin/echo");
        let args_json =
            serde_json::to_string(&vec!["stream-bag-marker"]).expect("serialize args");
        let args_vid = mint_trusted(&mut store, &args_json);

        // ── Node 1: trusted process.exec (CLEAN ALLOW) ───────────────────────
        let plan_node1 = PlanNode {
            sink: SinkId("process.exec".into()),
            args: vec![
                PlanArg {
                    name: "command".into(),
                    value_id: command_vid,
                },
                PlanArg {
                    name: "args".into(),
                    value_id: args_vid,
                },
            ],
        };
        let effect_id1 = Uuid::new_v4();
        let decision1 = executor::submit_plan_node(
            session_id,
            effect_id1,
            &plan_node1,
            &store,
            &SessionStatus::Active,
            &runtime_core::SessionPolicy::allow_all(),
        );
        assert_eq!(
            decision1,
            ExecutorDecision::Allowed,
            "Node1 trusted process.exec must Allow"
        );

        let ws_dir = fresh_workspace("bag");
        let workspace_root = WorkspaceRoot::open(&ws_dir).expect("open workspace root");

        let (exec_event_id, exec_hash, combined_output) = invoke_process_exec(
            &conn,
            TEST_KEY,
            &store,
            session_id,
            effect_id1,
            &plan_node1,
            &workspace_root,
            root_id,
            &root_hash,
        )
        .await
        .expect("invoke_process_exec must succeed for trusted /bin/echo");

        assert!(
            combined_output.contains("stream-bag-marker"),
            "captured output must contain marker, got: {combined_output:?}"
        );

        // Anti-stapling: process_exited is durably in the DAG before mint.
        {
            let locked = conn.lock().expect("lock conn");
            let dag_event =
                find_event_by_type(&locked, &session_id.to_string(), "process_exited")
                    .expect("query process_exited")
                    .expect("process_exited must exist");
            assert_eq!(dag_event.id, exec_event_id);
        }

        let output_value_id =
            mint_from_exec(&mut store, session_id, combined_output, exec_event_id)
                .expect("mint_from_exec must succeed");

        // Genuine provenance: chain root is process_exited event id.
        let minted = store
            .resolve(&output_value_id)
            .expect("output_value_id must resolve")
            .clone();
        assert_eq!(
            minted.provenance_chain,
            vec![exec_event_id],
            "mint_from_exec provenance_chain must be exactly [process_exited]"
        );
        assert!(minted.taint.contains(&TaintLabel::ExternalUntrusted));
        assert!(minted.taint.contains(&TaintLabel::ExecRaw));

        // ── Bag intermediate (F-01 worker path): store under out_0 ───────────
        let mut bag: HashMap<String, ValueId> = HashMap::new();
        let branch = apply_stream_decision(
            &mut bag,
            0,
            &ExecutorDecision::Allowed,
            Some(output_value_id.clone()),
        );
        assert_eq!(branch, StreamBranch::Continue);
        assert_eq!(
            bag.get("out_0"),
            Some(&output_value_id),
            "F-01 bag must store genuine mint_from_exec handle under out_0"
        );

        // ── Node 2: plan surface places bag handle into process.exec/command ─
        // (STREAM-02 adjacency — not a stapled sink-local mint.)
        let bag_handle = bag
            .get("out_0")
            .expect("bag must carry out_0")
            .clone();
        let plan_node2 = PlanNode {
            sink: SinkId("process.exec".into()),
            args: vec![PlanArg {
                name: "command".into(),
                value_id: bag_handle,
            }],
        };
        let effect_id2 = Uuid::new_v4();
        let decision2 = executor::submit_plan_node(
            session_id,
            effect_id2,
            &plan_node2,
            &store,
            &SessionStatus::Active,
            &runtime_core::SessionPolicy::allow_all(),
        );

        let anchor = match decision2 {
            ExecutorDecision::BlockedPendingConfirmation { anchors } => {
                assert_eq!(anchors.len(), 1, "exactly one blocked arg (command)");
                let blocked = anchors.into_iter().next().expect("one anchor");
                assert_eq!(blocked.anchor.arg, "command");
                assert_eq!(blocked.anchor.sink.0, "process.exec");
                blocked.anchor
            }
            other => panic!(
                "expected BlockedPendingConfirmation for bagged exec-output as \
                 process.exec/command, got {other:?}"
            ),
        };

        // Genuine-taint backstop: provenance root is the process_exited event.
        assert_eq!(
            anchor.provenance_chain[0], exec_event_id,
            "GENUINE-TAINT BACKSTOP: provenance_chain[0] must equal process_exited id"
        );
        assert_eq!(
            anchor.read_event_id, exec_event_id,
            "anchor.read_event_id must equal process_exited event id"
        );

        // Stream branch on Block: stop, no re-submit.
        let mut bag_after = bag.clone();
        let stop = apply_stream_decision(
            &mut bag_after,
            1,
            &ExecutorDecision::BlockedPendingConfirmation {
                anchors: vec![],
            },
            None,
        );
        assert_eq!(stop, StreamBranch::StopBlocked);
        assert_eq!(
            bag_after.get("out_0"),
            Some(&output_value_id),
            "Block must not mutate prior bag entries; no re-submit recovery path"
        );

        // Durable block + verify_chain (mirrors s9_process_exec_block spine).
        let block_event = Event::sink_blocked(
            Uuid::new_v4(),
            Some(exec_event_id),
            session_id,
            Utc::now(),
            vec![anchor],
            None,
            vec!["command".to_string()],
        );
        {
            let locked = conn.lock().expect("lock conn");
            append_event(&locked, TEST_KEY, &block_event, Some(&exec_hash))
                .expect("append sink_blocked");
        }

        let locked = conn.lock().expect("lock conn");
        let persisted_block =
            find_event_by_type(&locked, &session_id.to_string(), "sink_blocked")
                .expect("query sink_blocked")
                .expect("durable sink_blocked must exist");
        assert_eq!(persisted_block.id, block_event.id);
        assert!(
            find_event_by_type(&locked, &session_id.to_string(), "sink_executed")
                .expect("query sink_executed")
                .is_none(),
            "no sink_executed — blocked node has no effect"
        );
        assert!(
            verify_chain(&locked, &session_id.to_string(), TEST_KEY),
            "verify_chain must be true: session_created -> process_exited -> sink_blocked"
        );
    }
}
