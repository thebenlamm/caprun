/// display — the ONE shared control-char neutralizer for any attacker-
/// influenceable literal about to be written to a human's terminal.
///
/// Neutralize terminal control characters in an attacker-influenceable literal
/// BEFORE it is written to the confirm prompt (WG-8 / T-44-19, mirrors the U1 /
/// VIEW-01 viewer discipline): a tainted refspec / remote / filename could embed
/// ANSI escapes (ESC `0x1b`, the CSI sequence) or other C0/C1 control bytes to
/// SPOOF or HIDE audit lines in the human's terminal. Every `char::is_control()`
/// byte (C0 incl. ESC/CR/LF/TAB, the C1 range, and DEL) is replaced with a
/// visible `\xNN` / `\u{NNNN}` escape. In addition, the Unicode "Trojan Source"
/// spoofing class (CVE-2021-42574) — BiDi overrides/embeddings/isolates and
/// zero-width joiners — is ALSO escaped even though those codepoints are
/// category `Cf` (format), not `Cc` (control), so `char::is_control()` returns
/// false for them: `U+200B..=U+200F` (ZWSP/ZWNJ/ZWJ/LRM/RLM), `U+202A..=U+202E`
/// (LRE/RLE/PDF/LRO/RLO), `U+2066..=U+2069` (LRI/RLI/FSI/PDI), and `U+FEFF`
/// (ZWNBSP/BOM). A tainted refspec/remote/filename carrying `U+202E` would
/// otherwise reach the human's confirm terminal with its visual order reversed —
/// a decision-surface spoof this fn exists to prevent. Ordinary printable text —
/// including non-ASCII UTF-8 — is preserved verbatim so the human still reads
/// the real value. Pure/deterministic; performs no I/O.
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
        if c.is_control() || is_format_spoof_char(c) {
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

/// The Unicode "Trojan Source" (CVE-2021-42574) spoofing class: BiDi
/// overrides/embeddings/isolates and zero-width joiners. These are category
/// `Cf` (format), NOT `Cc` (control), so `char::is_control()` misses them, yet
/// they reorder or hide terminal text — a decision-surface spoof on the confirm
/// prompt. Neutralized alongside the control chars.
fn is_format_spoof_char(c: char) -> bool {
    matches!(
        c as u32,
        0x200B..=0x200F   // ZWSP, ZWNJ, ZWJ, LRM, RLM
        | 0x202A..=0x202E // LRE, RLE, PDF, LRO, RLO
        | 0x2066..=0x2069 // LRI, RLI, FSI, PDI
        | 0xFEFF          // ZWNBSP / BOM
    )
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

    /// The Trojan-Source (CVE-2021-42574) spoofing class — BiDi overrides,
    /// isolates, and zero-width joiners (Unicode category `Cf`, which
    /// `char::is_control()` does NOT catch) — is escaped to a visible `\u{NNNN}`
    /// so a tainted refspec/remote cannot reorder or hide the confirm prompt.
    #[test]
    fn neutralize_escapes_bidi_and_zero_width_spoof_chars() {
        // RIGHT-TO-LEFT OVERRIDE (the canonical Trojan-Source char).
        assert_eq!(neutralize_control_chars("\u{202e}"), "\\u{202e}");
        assert!(!neutralize_control_chars("a\u{202e}b").contains('\u{202e}'));
        // Zero-width space, ZWJ, BOM, and a BiDi isolate.
        assert_eq!(neutralize_control_chars("\u{200b}"), "\\u{200b}");
        assert_eq!(neutralize_control_chars("\u{200d}"), "\\u{200d}");
        assert_eq!(neutralize_control_chars("\u{feff}"), "\\u{feff}");
        assert_eq!(neutralize_control_chars("\u{2066}"), "\\u{2066}");
        // A realistic spoofed remote: the ESC-free BiDi override is caught.
        let spoofed = neutralize_control_chars("https://evil.com/\u{202e}gpk.git");
        assert!(!spoofed.contains('\u{202e}'), "raw RLO must not survive");
        assert!(spoofed.contains("\\u{202e}"));
    }

    /// Ordinary format-adjacent but non-spoofing text is preserved: a normal
    /// combining mark and a currency sign are NOT in the spoof set.
    #[test]
    fn neutralize_preserves_non_spoof_non_ascii() {
        assert_eq!(neutralize_control_chars("e\u{0301}"), "e\u{0301}"); // combining acute
        assert_eq!(neutralize_control_chars("€"), "€");
    }
}
