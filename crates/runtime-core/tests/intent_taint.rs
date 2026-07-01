/// intent_taint.rs — serde round-trip for CaprunIntent + TaintLabel::is_untrusted truth table
///
/// Covers:
///   PLAN-02/PLAN-03: CaprunIntent typed enum serializes/deserializes correctly
///   HARD-02: TaintLabel::is_untrusted() returns the correct value for all 7 variants
use runtime_core::{CaprunIntent, TaintLabel};

// ── CaprunIntent serde ────────────────────────────────────────────────────────

#[test]
fn caprun_intent_serde_round_trip() {
    let intent = CaprunIntent::SendEmailSummary {
        recipient: "boss@company.com".into(),
    };
    let json = serde_json::to_string(&intent).expect("CaprunIntent serializes");
    let back: CaprunIntent = serde_json::from_str(&json).expect("CaprunIntent deserializes");
    assert_eq!(intent, back, "CaprunIntent serde round-trip must be lossless");
}

// ── TaintLabel::is_untrusted truth table ─────────────────────────────────────

#[test]
fn is_untrusted_user_trusted_returns_false() {
    assert!(
        !TaintLabel::UserTrusted.is_untrusted(),
        "UserTrusted is trusted provenance — must NOT block"
    );
}

#[test]
fn is_untrusted_local_workspace_returns_false() {
    assert!(
        !TaintLabel::LocalWorkspace.is_untrusted(),
        "LocalWorkspace is trusted provenance — must NOT block"
    );
}

#[test]
fn is_untrusted_external_untrusted_returns_true() {
    assert!(TaintLabel::ExternalUntrusted.is_untrusted());
}

#[test]
fn is_untrusted_email_raw_returns_true() {
    assert!(TaintLabel::EmailRaw.is_untrusted());
}

#[test]
fn is_untrusted_pdf_raw_returns_true() {
    assert!(TaintLabel::PdfRaw.is_untrusted());
}

#[test]
fn is_untrusted_llm_generated_returns_true() {
    assert!(TaintLabel::LlmGenerated.is_untrusted());
}

#[test]
fn is_untrusted_worker_extracted_returns_true() {
    assert!(TaintLabel::WorkerExtracted.is_untrusted());
}
