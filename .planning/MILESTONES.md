# Milestones

## v1.0 MVP — AgentOS v0 (Shipped: 2026-06-30)

**Phases completed:** 4 phases, 15 plans, 16 tasks

**Key accomplishments:**

- **Substrate foundation (Phase 1):** Cargo virtual workspace + `runtime-core` pure domain types — `ValueNode` carries the literal+provenance+taint triple from the first commit, 3-class Effect enum, and the broker `submit_plan_node` API locked to `PlanNode{sink, args}` with a structural no-bypass gate.
- **Security design gate (Phase 2):** `DESIGN-taint-model.md` + `DESIGN-plan-executor.md` — formal MUST/MUST NOT invariants (I0/I1/I2), the genuine-taint requirement, monotonic propagation rules, the hardcoded email.send sensitivity map, and the literal-value confirmation UX. Hard-gated all executor code.
- **Kernel confinement & mediation (Phase 3):** namespaces + Landlock + seccomp worker confinement, broker reference monitor, and SCM_RIGHTS fd-pass fs adapter — proven by the no-LLM substrate demo (Linux-verified 29/29): a confined worker reads a file only via a broker-passed fd, landing as an unbroken `session_created → fd_granted → file_read` audit hash chain.
- **Deterministic I2 executor (Phase 4):** `crates/executor` — pure non-LLM decision function over a broker-owned `ValueStore` (sole taint writer) with the email.send sensitivity map; anti-stapling verified by negative grep.
- **Genuine-taint reader (Phase 4):** quarantined extractor (planner never sees raw text) + `mint_from_read` as the sole broker taint-mint site, with `provenance_chain` anchored to the real `file_read` Event.
- **§9 acceptance test = v0 DONE (Phase 4):** end-to-end value-injection scenario blocks a tainted address at a mediated sink with literal-value confirmation; the two-sided backstop (`provenance_chain[0] == read_event_id`) fails for any stapled-taint implementation. `cargo test --workspace` = 51 green.

---
