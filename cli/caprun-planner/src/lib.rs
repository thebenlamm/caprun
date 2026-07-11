//! `caprun-planner` — the out-of-process LLM sidecar (PLANNER-03/04).
//!
//! This lib target exists so `openai.rs`'s pure helpers
//! (`build_chat_request`, `extract_tool_arguments`) can be unit-tested with
//! `cargo test -p caprun-planner --lib` without a live network call, exactly
//! as Task 1's `<verify>` step runs. `main.rs` (the bin target, Task 2) uses
//! this same module via `caprun_planner::openai`.

pub mod openai;
