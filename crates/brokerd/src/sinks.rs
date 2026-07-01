/// sinks — mediated sink dispatch modules
///
/// Each sub-module implements the post-confirmation dispatch target for a single
/// sink. The stub variant (used in v0) records the would-be invocation to the
/// audit DAG and performs no network or filesystem action.
///
/// The executor evaluates the plan node and blocks if taint is present; the
/// broker then builds a ConfirmationPrompt (approval.rs) and delivers it via
/// FAMP. Only after human confirmation does the broker call the sink.
/// For v0 the §9 held-out test never reaches this code because the executor
/// blocks first — the stub is exercised by unit tests that call it directly.

pub mod email_send;
pub mod file_create;
