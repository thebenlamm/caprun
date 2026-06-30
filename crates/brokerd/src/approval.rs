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
/// # Arguments
/// * `literal_value`   — the exact literal string from the broker-owned ValueRecord.
/// * `taint`           — taint labels carried by the blocked value.
/// * `source_event_id` — the Event id of the file_read (or equivalent) that
///                       introduced this value; the §9 test asserts this is the
///                       same Event stored in the audit DAG.
///
/// # Canonicalisation (v0)
/// Trims surrounding ASCII whitespace only. For the simple §9 case
/// (`"accounts@ev1l.com"`) raw == canonical. Punycode/homoglyph/RTL
/// normalisation are post-v0 extensions documented in DESIGN §Canonicalisation.
///
/// # Domain extraction
/// Returns the portion after the last `@`. If no `@` is present (malformed
/// address) the domain is the empty string.
///
/// # known_contact
/// Always `false` in v0. Post-v0: look up in user-trusted contact store.
pub fn build_confirmation_prompt(
    literal_value: String,
    taint: Vec<TaintLabel>,
    source_event_id: Uuid,
) -> ConfirmationPrompt {
    let canonical_address = literal_value.trim().to_string();
    let domain = canonical_address
        .rfind('@')
        .map(|pos| canonical_address[pos + 1..].to_string())
        .unwrap_or_default();

    ConfirmationPrompt {
        raw_recipient: literal_value,
        canonical_address,
        domain,
        known_contact: false,
        source_event_id,
        taint,
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
