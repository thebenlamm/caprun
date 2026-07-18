/// display — the ONE shared control-char neutralizer for any attacker-
/// influenceable literal about to be written to a human's terminal.
///
/// Neutralize terminal control characters in an attacker-influenceable literal
/// BEFORE it is written to the confirm prompt (WG-8 / T-44-19, mirrors the U1 /
/// VIEW-01 viewer discipline): a tainted refspec / remote / filename could embed
/// ANSI escapes (ESC `0x1b`, the CSI sequence) or other C0/C1 control bytes to
/// SPOOF or HIDE audit lines in the human's terminal. Every `char::is_control()`
/// byte (C0 incl. ESC/CR/LF/TAB, the C1 range, and DEL) is replaced with a
/// visible `\xNN` / `\u{NNNN}` escape; ordinary printable text — including
/// non-ASCII UTF-8 — is preserved verbatim so the human still reads the real
/// value. Pure/deterministic; performs no I/O.
///
/// This is the single shared implementation reachable by BOTH
/// `brokerd::confirmation` (the confirm prompt) and the `cli/caprun` read-only
/// audit-DAG viewer (VIEW-01, Plan 45-03). Extracted from the formerly-private
/// `confirmation::neutralize_control_chars` so a second copy cannot drift weaker
/// (RESEARCH "Don't Hand-Roll": two copies drift). The anti-drift test in
/// `confirmation.rs` asserts both callers resolve to this ONE implementation.
pub fn neutralize_control_chars(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_control() {
            let cp = c as u32;
            if cp <= 0xff {
                out.push_str(&format!("\\x{cp:02x}"));
            } else {
                out.push_str(&format!("\\u{{{cp:04x}}}"));
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The canonical T-44-19 case: an embedded ANSI CSI sequence's ESC byte is
    /// escaped to a visible `\x1b` literal, and the surrounding printable bytes
    /// (`a`, `[`, `2`, `K`, `b`) are preserved intact.
    #[test]
    fn neutralize_escapes_esc_preserves_surrounding_printables() {
        let out = neutralize_control_chars("a\x1b[2Kb");
        assert_eq!(out, "a\\x1b[2Kb");
        assert!(!out.contains('\u{1b}'), "raw ESC must not survive");
    }

    /// Every C0 control byte (CR, LF, TAB, ESC) and DEL is escaped to `\xNN`.
    #[test]
    fn neutralize_escapes_all_c0_and_del() {
        assert_eq!(neutralize_control_chars("\r"), "\\x0d");
        assert_eq!(neutralize_control_chars("\n"), "\\x0a");
        assert_eq!(neutralize_control_chars("\t"), "\\x09");
        assert_eq!(neutralize_control_chars("\x1b"), "\\x1b");
        assert_eq!(neutralize_control_chars("\x7f"), "\\x7f");
    }

    /// A C1 control codepoint (e.g. NEL `\u{0085}`, > 0xff after the C1 range is
    /// still ≤ 0xff so uses `\xNN`; a higher control codepoint uses `\u{NNNN}`).
    #[test]
    fn neutralize_escapes_c1_range() {
        // NEL (U+0085) is a control char with codepoint 0x85 (≤ 0xff → \xNN).
        assert_eq!(neutralize_control_chars("\u{0085}"), "\\x85");
        // A control codepoint above 0xff renders as \u{NNNN} (e.g. U+009F is
        // 0x9f, still ≤ 0xff; there are no assigned control chars > 0xff, so
        // exercise the branch structurally via the ≤0xff C1 case above).
    }

    /// Ordinary printable ASCII and non-ASCII UTF-8 are preserved byte-for-byte.
    #[test]
    fn neutralize_preserves_printable_and_utf8() {
        assert_eq!(neutralize_control_chars("café"), "café");
        assert_eq!(neutralize_control_chars("→"), "→");
        assert_eq!(
            neutralize_control_chars("https://github.com/o/r.git"),
            "https://github.com/o/r.git"
        );
    }

    /// An empty string returns empty; a control-free string returns itself.
    #[test]
    fn neutralize_empty_and_control_free_unchanged() {
        assert_eq!(neutralize_control_chars(""), "");
        assert_eq!(neutralize_control_chars("plain text 123"), "plain text 123");
    }
}
