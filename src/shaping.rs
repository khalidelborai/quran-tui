use std::sync::OnceLock;

use arabic_reshaper::ArabicReshaper;
use unicode_bidi::BidiInfo;

static ARABIC_RESHAPER: OnceLock<ArabicReshaper> = OnceLock::new();

// The TUI renders ayahs as terminal-native text, so the final visible font
// comes from the user's terminal configuration rather than this app. See
// README.md for the recommended Quran-oriented font stack.
pub(crate) fn shape(text: &str) -> String {
    let normalized = normalize_for_display(text);
    let reshaper = ARABIC_RESHAPER.get_or_init(ArabicReshaper::default);
    let shaped = reshaper.reshape(&normalized);
    let bidi_info = BidiInfo::new(&shaped, None);

    bidi_info
        .paragraphs
        .iter()
        .map(|paragraph| bidi_info.reorder_line(paragraph, paragraph.range.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn normalize_for_display(text: &str) -> String {
    let filtered: String = text.chars().filter(|ch| !is_ignored_format(*ch)).collect();
    trim_ascii_edges(&filtered).to_string()
}

pub(crate) fn visual_width(text: &str) -> usize {
    normalize_for_display(text)
        .chars()
        .filter(|ch| !is_zero_width_mark(*ch))
        .count()
        .max(1)
}

fn trim_ascii_edges(text: &str) -> &str {
    text.trim_matches(|ch: char| matches!(ch, ' ' | '\t' | '\n' | '\r'))
}

fn is_ignored_format(ch: char) -> bool {
    matches!(
        ch,
        '\u{feff}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

fn is_zero_width_mark(ch: char) -> bool {
    matches!(
        ch as u32,
        0x0300..=0x036F
            | 0x0610..=0x061A
            | 0x064B..=0x065F
            | 0x0670
            | 0x06D6..=0x06ED
            | 0x08D3..=0x08FF
            | 0xFE20..=0xFE2F
    )
}

#[cfg(test)]
mod tests {
    use super::{normalize_for_display, shape, visual_width};

    #[test]
    fn normalize_for_display_removes_bom_and_ascii_edge_whitespace() {
        assert_eq!(normalize_for_display("\u{feff}  بِسْمِ\n"), "بِسْمِ");
    }

    #[test]
    fn shape_preserves_paragraph_boundaries() {
        let shaped = shape("بسم\nالله");
        assert!(shaped.contains('\n'));
    }

    #[test]
    fn visual_width_ignores_extended_arabic_marks() {
        assert_eq!(visual_width("ر\u{08f0}"), visual_width("ر"));
    }
}
