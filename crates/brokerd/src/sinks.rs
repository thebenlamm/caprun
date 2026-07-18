/// sinks — mediated sink dispatch modules
///
/// Each sub-module implements the post-confirmation dispatch target for a
/// single sink. `file_create` and `email_smtp` both perform a REAL effect
/// (filesystem write / SMTP send) from a frozen `ResolvedArg` snapshot,
/// invoked only from `confirmation.rs::confirm()` after human confirmation.
/// (Phase 13 replaced the old `email.send` no-op stub module with
/// `email_smtp` as `confirm()`'s dispatch target; the stub module was
/// deleted — see `13-02-SUMMARY.md`.)
///
/// The executor evaluates the plan node and blocks if taint is present; the
/// broker then builds a ConfirmationPrompt (approval.rs) and delivers it via
/// FAMP. Only after human confirmation does the broker call the sink.

pub mod email_smtp;
pub mod file_create;
pub mod file_write;
pub mod git_commit;
pub mod process_exec;
