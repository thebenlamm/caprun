/// caprun — confined-worker orchestrator (Phase 3 demo harness)
///
/// Phase 3 Wave 0: stub binary. Wave 2 Plan 05 implements the full demo:
///   1. Start brokerd UDS server on abstract socket
///   2. Spawn caprun-worker with `Command::pre_exec` confinement
///   3. Wait for worker to request fd, read via fd, report read
///   4. Print audit DAG showing unbroken hash chain
///
/// See RESEARCH.md §Architecture Diagram and Pattern 1 (pre_exec confinement).

fn main() {}
