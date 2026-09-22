//! Small, dependency-free display helpers shared across panels.
//!
//! Kept separate from rendering code (SRP) so the logic is unit-testable
//! without an egui context.

/// Truncate `s` to at most `max_chars` **characters** (never bytes), appending
/// an ellipsis when truncation occurred.
///
/// # Why this exists
///
/// Slicing a `&str` by byte offset (`&s[..30]`) panics when the offset is not a
/// UTF-8 character boundary. Sequence strings normally contain only ASCII
/// residue codes, but they reach the UI from sources that do not validate
/// character encoding — notably `.dima` binary import, which verifies the
/// container format and CRC but not the encoding of the strings inside it. A
/// single multi-byte character in a `Variant::sequence` or a stitched
/// `HcsRegion::sequence` would therefore panic the render path, and because the
/// release profile sets `panic = "abort"`, that terminates the process rather
/// than unwinding.
///
/// Counting characters (rather than bytes) also makes the visual length of the
/// result match the requested budget, which byte slicing did not guarantee.
///
/// # Behaviour
///
/// - Returns `s` unchanged when it holds `max_chars` characters or fewer.
/// - Otherwise returns the first `max_chars` characters followed by `"..."`.
/// - Never panics, for any input, including `max_chars == 0`.
pub fn truncate_display(s: &str, max_chars: usize) -> String {
    // `char_indices().nth(max_chars)` yields the character at 0-based index
    // `max_chars` — i.e. the (max_chars + 1)-th character. Its presence proves
    // the string is longer than the budget, and its byte offset is exactly the
    // boundary after `max_chars` characters, so slicing there is always valid.
    match s.char_indices().nth(max_chars) {
        Some((boundary, _)) => {
            let mut out = String::with_capacity(boundary + 3);
            out.push_str(&s[..boundary]);
            out.push_str("...");
            out
        }
        None => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_short_ascii_unchanged() {
        assert_eq!(truncate_display("ACDEFG", 20), "ACDEFG");
    }

    #[test]
    fn returns_exact_length_unchanged() {
        // Exactly at the budget must NOT gain an ellipsis.
        assert_eq!(truncate_display("ACDEF", 5), "ACDEF");
    }

    #[test]
    fn truncates_longer_ascii() {
        assert_eq!(truncate_display("ABCDEFGHIJ", 4), "ABCD...");
    }

    #[test]
    fn truncates_on_char_boundary_for_multibyte() {
        // Greek letters are 2 bytes each: byte slicing at 3 would have panicked.
        let s = "αβγδεζ";
        assert_eq!(truncate_display(s, 3), "αβγ...");
    }

    #[test]
    fn does_not_truncate_when_bytes_exceed_but_chars_do_not() {
        // 5 characters, 10 bytes. The old byte-based check truncated here; the
        // character-based budget correctly leaves it intact.
        let s = "αβγδε";
        assert_eq!(s.len(), 10);
        assert_eq!(truncate_display(s, 8), s);
    }

    #[test]
    fn handles_emoji_and_wide_scalars() {
        // 4-byte scalars must not be split.
        let s = "🧬🧬🧬🧬";
        assert_eq!(truncate_display(s, 2), "🧬🧬...");
    }

    #[test]
    fn handles_zero_budget() {
        assert_eq!(truncate_display("ACDE", 0), "...");
        assert_eq!(truncate_display("", 0), "");
    }

    #[test]
    fn handles_empty_input() {
        assert_eq!(truncate_display("", 10), "");
    }

    #[test]
    fn never_panics_on_arbitrary_boundaries() {
        // Exhaustively exercise every budget against a mixed-width string.
        let s = "Aα🧬G\u{0301}Z";
        for n in 0..=s.chars().count() + 3 {
            let _ = truncate_display(s, n);
        }
    }
}
