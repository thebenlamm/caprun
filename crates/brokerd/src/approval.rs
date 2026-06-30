/// approval — literal-value confirmation prompt builder (REQ-approval-hook)
///
/// When the executor returns `BlockedPendingConfirmation`, the broker calls
/// `build_confirmation_prompt` to produce a `ConfirmationPrompt` that surfaces
/// the byte-exact recipient address (raw + canonical + domain + known_contact)
/// and the source event that introduced the value.
///
/// Design reference: DESIGN-plan-executor.md §Literal-Value Confirmation UX
///   "The confirmation must show the EXACT recipient address, not a category."
///
/// Security (T-04-04): the prompt shows raw_recipient == canonical_address for
/// ASCII no-display-name inputs, plus domain and known_contact=false.
///
/// Post-v0 extension: punycode, homoglyph, RTL, RFC 5322 display-name stripping
/// are future canonicalisations (v0: trim ASCII whitespace only).

use runtime_core::TaintLabel;
use uuid::Uuid;

/// The payload delivered to the human operator for a blocked plan node.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfirmationPrompt {
    /// The byte-exact recipient address as it appeared in the ValueRecord.
    pub raw_recipient: String,
    /// Canonicalised form of the address (v0: trim whitespace, ASCII only).
    pub canonical_address: String,
    /// Domain portion (substring after the last `@`).
    pub domain: String,
    /// Whether the address is in the user-trusted contact book (v0: always false).
    pub known_contact: bool,
    /// The Event id of the read that introduced this value into the DAG.
    pub source_event_id: Uuid,
    /// Taint labels propagated from the source Event's ValueRecord.
    pub taint: Vec<TaintLabel>,
}

/// Build a literal-value confirmation prompt for a blocked plan node.
///
/// # RED stub — implement in GREEN phase
pub fn build_confirmation_prompt(
    _literal_value: String,
    _taint: Vec<TaintLabel>,
    _source_event_id: Uuid,
) -> ConfirmationPrompt {
    // RED stub — returns wrong values so tests fail
    ConfirmationPrompt {
        raw_recipient: String::new(),
        canonical_address: String::new(),
        domain: String::new(),
        known_contact: true, // wrong — will fail known_contact assertion
        source_event_id: Uuid::nil(),
        taint: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use runtime_core::TaintLabel;

    #[test]
    fn build_confirmation_prompt_literal_fidelity() {
        let literal = "accounts@ev1l.com".to_string();
        let source_id = Uuid::new_v4();
        let taint = vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw];

        let prompt =
            build_confirmation_prompt(literal.clone(), taint.clone(), source_id);

        assert_eq!(prompt.raw_recipient, "accounts@ev1l.com");
        assert_eq!(prompt.canonical_address, "accounts@ev1l.com");
        assert_eq!(prompt.domain, "ev1l.com");
        assert!(!prompt.known_contact, "known_contact must be false in v0");
        assert_eq!(prompt.source_event_id, source_id);
        assert_eq!(prompt.taint, taint);
    }

    #[test]
    fn build_confirmation_prompt_trims_whitespace() {
        let literal = "  user@example.com  ".to_string();
        let source_id = Uuid::new_v4();

        let prompt =
            build_confirmation_prompt(literal.clone(), vec![], source_id);

        assert_eq!(prompt.raw_recipient, "  user@example.com  ");
        assert_eq!(prompt.canonical_address, "user@example.com");
        assert_eq!(prompt.domain, "example.com");
    }

    #[test]
    fn build_confirmation_prompt_no_at_sign_yields_empty_domain() {
        let literal = "notanemail".to_string();
        let source_id = Uuid::new_v4();

        let prompt = build_confirmation_prompt(literal, vec![], source_id);

        assert_eq!(prompt.domain, "", "malformed address → empty domain");
    }
}
