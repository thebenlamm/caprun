//! LIVE-07 non-hybrid CLI multi-node acceptance proof.

#[test]
fn live_acceptance_v1_10_cli_guard_present() {
    assert!(!env!("CARGO_BIN_EXE_caprun").is_empty());
    live_07_cli_multi_node_one_session_verify_chain();
}
