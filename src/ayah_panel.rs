use ratatui::layout::Rect;

use crate::shaping::{normalize_for_display, shape, visual_width};

type RenderKey = (String, u16);

pub(crate) struct AyahTextPanel {
    render_key: Option<RenderKey>,
    rendered_lines: Vec<String>,
}

impl AyahTextPanel {
    pub(crate) fn new() -> Self {
        Self {
            render_key: None,
            rendered_lines: vec![],
        }
    }

    pub(crate) fn clear(&mut self) {
        self.render_key = None;
        self.rendered_lines.clear();
    }

    pub(crate) fn update(&mut self, text: Option<&str>, area: Rect) {
        let Some(text) = text else {
            self.clear();
            return;
        };
        if area.width == 0 || area.height == 0 {
            return;
        }

        let text = sanitize_text(text);
        let key = (text.clone(), area.width);
        if self.render_key.as_ref() == Some(&key) {
            return;
        }

        self.render_key = Some(key);
        self.rendered_lines = wrap_ayah_text(&text, area.width)
            .into_iter()
            .map(|line| shape(&line))
            .collect();
    }

    pub(crate) fn rendered_lines(&self) -> &[String] {
        &self.rendered_lines
    }
}

fn sanitize_text(text: &str) -> String {
    normalize_for_display(text)
}

fn wrap_ayah_text(text: &str, area_width: u16) -> Vec<String> {
    let text = sanitize_text(text);
    if text.is_empty() {
        return vec![String::new()];
    }

    let max_line_units = usize::from(area_width.saturating_sub(8)).max(12);
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut pending_whitespace = String::new();

    for run in split_runs(&text) {
        if run.chars().all(char::is_whitespace) {
            if !current.is_empty() {
                pending_whitespace.push_str(run);
            }
            continue;
        }

        let candidate = if current.is_empty() {
            run.to_string()
        } else {
            format!("{current}{pending_whitespace}{run}")
        };

        if current.is_empty() || visual_width(&candidate) <= max_line_units || lines.len() >= 2 {
            current = candidate;
            pending_whitespace.clear();
            continue;
        }

        lines.push(current.trim_end().to_string());
        current = run.to_string();
        pending_whitespace.clear();
    }

    if lines.is_empty() || !current.is_empty() {
        lines.push(current.trim_end().to_string());
    }

    lines
}

fn split_runs(text: &str) -> Vec<&str> {
    let mut runs = Vec::new();
    let mut start = 0;
    let mut previous_kind = None;

    for (index, ch) in text.char_indices() {
        let is_whitespace = ch.is_whitespace();
        if let Some(previous_kind) = previous_kind
            && previous_kind != is_whitespace
        {
            runs.push(&text[start..index]);
            start = index;
        }
        previous_kind = Some(is_whitespace);
    }

    runs.push(&text[start..]);
    runs
}

#[cfg(test)]
mod tests {
    use super::{sanitize_text, split_runs, wrap_ayah_text};
    use crate::shaping::visual_width;

    #[test]
    fn sanitize_text_removes_bom() {
        assert_eq!(sanitize_text("\u{feff}بِسْمِ"), "بِسْمِ");
    }

    #[test]
    fn sanitize_text_preserves_internal_spacing() {
        assert_eq!(sanitize_text("  آية  آية  "), "آية  آية");
    }

    #[test]
    fn visual_units_ignore_arabic_diacritics() {
        assert_eq!(visual_width("ٱلرَّحۡمَٰنِ"), visual_width("الرحمن"));
    }

    #[test]
    fn wrap_ayah_text_keeps_short_text_on_one_line() {
        let lines = wrap_ayah_text("ٱلْحَمْدُ لِلَّهِ رَبِّ ٱلْعَٰلَمِينَ", 40);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn wrap_ayah_text_splits_longer_text_into_multiple_lines() {
        let lines = wrap_ayah_text("ذَٰلِكَ ٱلْكِتَٰبُ لَا رَيْبَ فِيهِ هُدًى لِّلْمُتَّقِينَ", 20);
        assert!(lines.len() >= 2);
    }

    #[test]
    fn wrap_ayah_text_preserves_multiple_spaces() {
        let lines = wrap_ayah_text("كلمة  كلمة", 40);
        assert_eq!(lines, vec!["كلمة  كلمة"]);
    }

    #[test]
    fn split_runs_keeps_separator_tokens() {
        assert_eq!(split_runs("a  b"), vec!["a", "  ", "b"]);
    }
}
